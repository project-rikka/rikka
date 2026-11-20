use std::f32::consts::TAU;
use std::marker::PhantomData;

use rikka_physics_math::{Mask, Wide};

use super::columns::columns_from;
use super::drive::Drive;
use super::spring::Spring;
use crate::solver::lanes::MAX_LANES;

pub(crate) const DRIVE_SLOTS: usize = 8;
pub(crate) const LINEAR_POSE: usize = 6;
pub(crate) const ANGULAR_POSE: usize = 7;

const PRESENT: u8 = 1;
const SPRING: u8 = 1 << 1;
const STIFFNESS_COEFFICIENT: u8 = 1 << 2;
const DAMPING_COEFFICIENT: u8 = 1 << 3;
const COEFFICIENTS: u64 = u64::from_ne_bytes([STIFFNESS_COEFFICIENT | DAMPING_COEFFICIENT; 8]);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DriveTable {
    stiffness: [f32; DRIVE_SLOTS],
    damping: [f32; DRIVE_SLOTS],
    max_effort: [f32; DRIVE_SLOTS],
    flags: [u8; DRIVE_SLOTS],
}

impl DriveTable {
    pub(crate) const EMPTY: Self = Self {
        stiffness: [0.0; DRIVE_SLOTS],
        damping: [0.0; DRIVE_SLOTS],
        max_effort: [0.0; DRIVE_SLOTS],
        flags: [0; DRIVE_SLOTS],
    };

    pub(crate) fn get(&self, slot: usize) -> Option<Drive> {
        let flags = self.flags[slot];
        (flags & PRESENT != 0).then(|| Drive {
            spring: (flags & SPRING != 0).then(|| {
                Spring::from_parts(
                    [self.stiffness[slot], self.damping[slot]],
                    [
                        flags & STIFFNESS_COEFFICIENT != 0,
                        flags & DAMPING_COEFFICIENT != 0,
                    ],
                )
            }),
            max_effort: self.max_effort[slot],
        })
    }

    pub(crate) fn set(&mut self, slot: usize, drive: Option<Drive>) {
        let Some(drive) = drive else {
            self.flags[slot] = 0;
            self.stiffness[slot] = 0.0;
            self.damping[slot] = 0.0;
            self.max_effort[slot] = 0.0;
            return;
        };
        let ([stiffness, damping], [stiffness_coefficient, damping_coefficient]) = drive
            .spring
            .map_or(([0.0; 2], [false; 2]), Spring::to_parts);
        let bit = |on: bool, flag: u8| if on { flag } else { 0 };
        self.flags[slot] = PRESENT
            | bit(drive.spring.is_some(), SPRING)
            | bit(stiffness_coefficient, STIFFNESS_COEFFICIENT)
            | bit(damping_coefficient, DAMPING_COEFFICIENT);
        self.stiffness[slot] = stiffness;
        self.damping[slot] = damping;
        self.max_effort[slot] = drive.max_effort;
    }

    pub(crate) fn update(&mut self, slot: usize, initial: Drive, change: impl FnOnce(&mut Drive)) {
        let mut drive = self.get(slot).unwrap_or(initial);
        change(&mut drive);
        self.set(slot, Some(drive));
    }

    pub(crate) fn is_set(&self, slot: usize) -> bool {
        self.flags[slot] & PRESENT != 0
    }

    pub(crate) fn has_spring(&self, slot: usize) -> bool {
        self.flags[slot] & SPRING != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DriveSettingsW<W: Wide> {
    pub bias_rate: W,
    pub mass_scale: W,
    pub impulse_scale: W,
    pub max_impulse: W,
}

#[derive(Clone, Copy, Debug)]
struct SpringTerms<W: Wide> {
    hertz: W,
    ratio: W,
    damper_mass_scale: W,
    damper_impulse_scale: W,
}

#[derive(Debug)]
pub(crate) struct DriveLanes<'a, W: Wide> {
    tables: [&'a DriveTable; MAX_LANES],
    uniform: Option<[u8; DRIVE_SLOTS]>,
    coefficients: bool,
    h: f32,
    max_hertz: f32,
    lanes: PhantomData<W>,
}

impl<'a, W: Wide> DriveLanes<'a, W> {
    #[inline(always)]
    pub(crate) fn new(tables: [&'a DriveTable; MAX_LANES], h: f32, max_hertz: f32) -> Self {
        let first = u64::from_ne_bytes(tables[0].flags);
        let (mut any, mut all) = (0, u64::MAX);
        for table in tables.iter().take(W::LANES) {
            let flags = u64::from_ne_bytes(table.flags);
            any |= flags;
            all &= !(flags ^ first);
        }
        Self {
            tables,
            uniform: (all == u64::MAX).then_some(tables[0].flags),
            coefficients: any & COEFFICIENTS != 0,
            h,
            max_hertz,
            lanes: PhantomData,
        }
    }

    #[inline(always)]
    fn column<const N: usize>(&self, first: usize, pick: fn(&DriveTable) -> &[f32]) -> [W; N] {
        let mut rows: [&[f32]; MAX_LANES] = [&[]; MAX_LANES];
        for (row, table) in rows.iter_mut().zip(&self.tables).take(W::LANES) {
            *row = pick(table);
        }
        columns_from(&rows, first)
    }

    #[inline(always)]
    fn mask(&self, slot: usize, flag: u8) -> W::Mask {
        if let Some(flags) = self.uniform {
            return W::Mask::splat(flags[slot] & flag != 0);
        }
        let mut values = [0.0f32; MAX_LANES];
        for (value, table) in values.iter_mut().zip(&self.tables).take(W::LANES) {
            if table.flags[slot] & flag != 0 {
                *value = 1.0;
            }
        }
        W::load(&values).simd_gt(W::default())
    }

    #[inline(always)]
    fn coefficient_terms(&self, slot: usize, stiffness: W, damping: W, mass: W) -> SpringTerms<W> {
        let zero = W::default();
        let one = W::one();
        let massive = mass.simd_gt(zero);
        let damping_coefficient = self.mask(slot, DAMPING_COEFFICIENT);
        let hertz = W::select(
            self.mask(slot, STIFFNESS_COEFFICIENT),
            ((stiffness / mass).sqrt() / W::splat(TAU)).masked(massive),
            stiffness,
        );
        let omega = W::splat(TAU) * hertz;
        let ratio = W::select(
            damping_coefficient,
            (damping / (W::splat(2.0) * mass * omega)).masked(omega.simd_gt(zero) & massive),
            damping,
        );
        let a2 = W::splat(self.h) * damping / mass;
        let a3 = one / (one + a2);
        let damper = damping_coefficient & damping.simd_gt(zero) & massive;
        SpringTerms {
            hertz,
            ratio,
            damper_mass_scale: (a2 * a3).masked(damper),
            damper_impulse_scale: a3.masked(damper),
        }
    }

    #[inline(always)]
    pub(crate) fn settings<const N: usize>(
        &self,
        first: usize,
        masses: [W; N],
    ) -> [DriveSettingsW<W>; N] {
        let stiffness: [W; N] = self.column(first, |table| &table.stiffness);
        let damping: [W; N] = self.column(first, |table| &table.damping);
        let max_effort: [W; N] = self.column(first, |table| &table.max_effort);
        let zero = W::default();
        let mut out = [DriveSettingsW {
            bias_rate: zero,
            mass_scale: zero,
            impulse_scale: zero,
            max_impulse: zero,
        }; N];
        for (index, settings) in out.iter_mut().enumerate() {
            *settings = self.slot_settings(
                first + index,
                stiffness[index],
                damping[index],
                max_effort[index],
                masses[index],
            );
        }
        out
    }

    #[inline(always)]
    fn slot_settings(
        &self,
        slot: usize,
        stiffness: W,
        damping: W,
        max_effort: W,
        mass: W,
    ) -> DriveSettingsW<W> {
        let zero = W::default();
        let one = W::one();
        let h = W::splat(self.h);
        let terms = if self.coefficients {
            self.coefficient_terms(slot, stiffness, damping, mass)
        } else {
            SpringTerms {
                hertz: stiffness,
                ratio: damping,
                damper_mass_scale: zero,
                damper_impulse_scale: zero,
            }
        };
        let omega = W::splat(TAU) * terms.hertz.min(W::splat(self.max_hertz));
        let a1 = W::splat(2.0) * terms.ratio + h * omega;
        let a2 = h * omega * a1;
        let a3 = one / (one + a2);
        let oscillating = terms.hertz.simd_gt(zero);
        let spring = self.mask(slot, SPRING);
        DriveSettingsW {
            bias_rate: (omega / a1).masked(oscillating & spring),
            mass_scale: W::select(
                spring,
                W::select(oscillating, a2 * a3, terms.damper_mass_scale),
                one,
            ),
            impulse_scale: W::select(oscillating, a3, terms.damper_impulse_scale).masked(spring),
            max_impulse: W::select(
                max_effort.simd_ge(W::splat(f32::MAX)),
                W::splat(f32::MAX),
                max_effort * h,
            ),
        }
    }
}
