package sh.rikka.client.mods.animation;

import org.joml.Matrix4f;
import org.joml.Matrix4fc;

final class ThirdPersonPose {
	private static final float HAND_OFFSET = 1.0F / 16.0F;
	private static final float HAND_HEIGHT = 2.0F / 16.0F;
	private static final float HAND_REACH = -10.0F / 16.0F;
	private static final Matrix4fc BLOCK_INSERT = buildBlockInsert();
	private static final Matrix4fc MIRRORED_BLOCK_INSERT = SpritePlacement.mirrored(BLOCK_INSERT);
	private static final Matrix4fc[] TARGETS = targets(false);
	private static final Matrix4fc[] BLOCKING_TARGETS = targets(true);

	private ThirdPersonPose() {
	}

	static Matrix4f exact(ItemFamily family, boolean blocking, Matrix4fc display) {
		return SpritePlacement.fit((blocking ? BLOCKING_TARGETS : TARGETS)[family.ordinal()], display);
	}

	static Matrix4fc blockInsert(boolean mirrored) {
		return mirrored ? MIRRORED_BLOCK_INSERT : BLOCK_INSERT;
	}

	private static Matrix4fc[] targets(boolean blocking) {
		ItemFamily[] families = ItemFamily.values();
		Matrix4fc[] targets = new Matrix4fc[families.length];
		Matrix4f toLegacy = armToModernFrame().invert().mul(armToLegacyFrame());
		for (ItemFamily family : families) {
			targets[family.ordinal()] = new Matrix4f(toLegacy).mul(chain(family, blocking)).mul(SpritePlacement.ITEM);
		}
		return targets;
	}

	private static Matrix4f buildBlockInsert() {
		Matrix4f modern = armToModernFrame();
		Matrix4f legacy = armToLegacyFrame();
		return new Matrix4f(modern).invert().mul(legacy).mul(block()).mul(new Matrix4f(legacy).invert()).mul(modern);
	}

	private static Matrix4f armToModernFrame() {
		return new Matrix4f()
			.rotateX(SpritePlacement.radians(-90.0F))
			.rotateY(SpritePlacement.radians(180.0F))
			.translate(HAND_OFFSET, HAND_HEIGHT, HAND_REACH);
	}

	private static Matrix4f armToLegacyFrame() {
		return new Matrix4f().translate(-0.0625F, 0.4375F, 0.0625F);
	}

	private static Matrix4f block() {
		return new Matrix4f()
			.translate(0.05F, 0.0F, -0.1F)
			.rotateY(SpritePlacement.radians(-50.0F))
			.rotateX(SpritePlacement.radians(-10.0F))
			.rotateZ(SpritePlacement.radians(-60.0F));
	}

	private static Matrix4f chain(ItemFamily family, boolean blocking) {
		Matrix4f chain = new Matrix4f();
		switch (family) {
			case GENERATED -> chain
				.translate(0.25F, 0.1875F, -0.1875F)
				.scale(0.375F)
				.rotateZ(SpritePlacement.radians(60.0F))
				.rotateX(SpritePlacement.radians(-90.0F))
				.rotateZ(SpritePlacement.radians(20.0F));
			case BOW -> chain
				.translate(0.0F, 0.125F, 0.3125F)
				.rotateY(SpritePlacement.radians(-20.0F))
				.scale(0.625F, -0.625F, 0.625F)
				.rotateX(SpritePlacement.radians(-100.0F))
				.rotateY(SpritePlacement.radians(45.0F));
			case HANDHELD, HANDHELD_ROD -> {
				if (family.rotatesAround()) {
					chain.rotateZ(SpritePlacement.radians(180.0F)).translate(0.0F, -0.125F, 0.0F);
				}
				if (blocking) {
					chain.mul(block());
				}
				chain
					.translate(0.0F, 0.1875F, 0.0F)
					.scale(0.625F, -0.625F, 0.625F)
					.rotateX(SpritePlacement.radians(-100.0F))
					.rotateY(SpritePlacement.radians(45.0F));
			}
		}
		return chain;
	}
}
