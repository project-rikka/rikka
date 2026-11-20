use rikka_physics_collision::ContactManifold;

use super::context::ContactContext;

const SLOT_SHIFT: u32 = 27;
const CONTACT_MASK: u32 = (1 << SLOT_SHIFT) - 1;

#[inline]
pub(crate) fn encode(contact: u32, slot: u32) -> u32 {
    debug_assert!(contact <= CONTACT_MASK && slot < 32);
    contact | slot << SLOT_SHIFT
}

#[inline(always)]
pub(crate) fn contact_of(reference: u32) -> u32 {
    reference & CONTACT_MASK
}

#[inline(always)]
pub(crate) unsafe fn manifold<'a>(
    ctx: &ContactContext<'a>,
    reference: u32,
) -> &'a mut ContactManifold {
    let contact = contact_of(reference) as usize;
    let slot = reference >> SLOT_SHIFT;
    if slot == 0 {
        unsafe { &mut ctx.geometry.get(contact).manifold }
    } else {
        let extra = ctx.headers[contact].extra;
        unsafe {
            ctx.extras[extra.worker as usize]
                .shared()
                .get((extra.start + slot - 1) as usize)
        }
    }
}
