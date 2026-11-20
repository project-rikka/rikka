use rikka_renderer::Joined;
use rikka_types::RoundedRect;

use super::placed::Piece;

const FILLETS: [u64; 2] = [1 << 62, 1 << 61];

pub(super) fn pieces(shapes: &[(u64, RoundedRect)], into: &mut Vec<(u64, Piece)>) {
    into.clear();
    into.extend(shapes.iter().map(|&(key, shape)| (key, Piece::lone(shape))));
    for host in 0..shapes.len() {
        for limb in 0..shapes.len() {
            if host == limb || into[host].1.joined.is_some() || into[limb].1.joined.is_some() {
                continue;
            }
            let Some(joined) = Joined::new(shapes[host].1, shapes[limb].1) else {
                continue;
            };
            into[host].1 = Piece {
                shape: joined.host(),
                hollow: None,
                joined: Some(joined),
            };
            into[limb].1 = Piece {
                shape: joined.limb(),
                hollow: None,
                joined: Some(joined),
            };
            let limb_key = shapes[limb].0;
            for (fillet, bit) in joined.fillets().into_iter().zip(FILLETS) {
                let Some((square, hollow)) = fillet else {
                    continue;
                };
                into.push((
                    limb_key ^ bit,
                    Piece {
                        shape: RoundedRect::rect(square),
                        hollow: Some(hollow),
                        joined: Some(joined),
                    },
                ));
            }
        }
    }
}
