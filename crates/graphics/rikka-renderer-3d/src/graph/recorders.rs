use std::marker::PhantomData;
use std::mem::MaybeUninit;

use crate::error::Result;
use crate::graph::builder::PassScope;
use crate::record::Record;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

type Unit = MaybeUninit<u128>;

const UNIT: usize = size_of::<Unit>();

type Call = unsafe fn(*mut u8, &mut Record<'_>, &mut GpuFrameCtx<'_>, &PassScope<'_>) -> Result<()>;

type Discard = unsafe fn(*mut u8);

struct Entry {
    at: usize,
    call: Call,
    discard: Discard,
    pending: bool,
}

#[derive(Default)]
pub(crate) struct RecorderStore {
    units: Vec<Unit>,
    entries: Vec<Entry>,
}

impl RecorderStore {
    fn clear(&mut self) {
        for entry in self.entries.iter_mut().filter(|entry| entry.pending) {
            entry.pending = false;
            unsafe { (entry.discard)(self.units[entry.at..].as_mut_ptr().cast()) };
        }
        self.entries.clear();
        self.units.clear();
    }
}

pub(super) struct Recorders<'a> {
    store: RecorderStore,
    lifetime: PhantomData<&'a ()>,
}

unsafe fn call_recorder<F>(
    at: *mut u8,
    record: &mut Record<'_>,
    ctx: &mut GpuFrameCtx<'_>,
    scope: &PassScope<'_>,
) -> Result<()>
where
    F: FnOnce(&mut Record<'_>, &mut GpuFrameCtx<'_>, &PassScope<'_>) -> Result<()>,
{
    let recorder = unsafe { at.cast::<F>().read() };
    recorder(record, ctx, scope)
}

unsafe fn discard_recorder<F>(at: *mut u8) {
    unsafe { at.cast::<F>().drop_in_place() };
}

impl<'a> Recorders<'a> {
    pub(super) fn new(mut store: RecorderStore) -> Self {
        store.clear();
        Self {
            store,
            lifetime: PhantomData,
        }
    }

    pub(super) fn push<F>(&mut self, recorder: F)
    where
        F: FnOnce(&mut Record<'_>, &mut GpuFrameCtx<'_>, &PassScope<'_>) -> Result<()> + 'a,
    {
        const {
            assert!(
                align_of::<F>() <= UNIT,
                "pass closures align to at most 16 bytes"
            )
        };
        let units = size_of::<F>().div_ceil(UNIT).max(1);
        let at = self.store.units.len();
        self.store.units.resize(at + units, MaybeUninit::uninit());
        unsafe {
            self.store.units[at..]
                .as_mut_ptr()
                .cast::<F>()
                .write(recorder)
        };
        self.store.entries.push(Entry {
            at,
            call: call_recorder::<F>,
            discard: discard_recorder::<F>,
            pending: true,
        });
    }

    pub(super) fn len(&self) -> usize {
        self.store.entries.len()
    }

    pub(super) fn run(
        &mut self,
        index: usize,
        record: &mut Record<'_>,
        ctx: &mut GpuFrameCtx<'_>,
        scope: &PassScope<'_>,
    ) -> Result<()> {
        let entry = &mut self.store.entries[index];
        assert!(entry.pending, "a pass records once");
        entry.pending = false;
        let at = self.store.units[entry.at..].as_mut_ptr().cast();
        unsafe { (entry.call)(at, record, ctx, scope) }
    }

    pub(super) fn into_store(mut self) -> RecorderStore {
        self.store.clear();
        std::mem::take(&mut self.store)
    }
}

impl Drop for Recorders<'_> {
    fn drop(&mut self) {
        self.store.clear();
    }
}
