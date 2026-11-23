package sh.rikka.client.mods.animation;

import net.minecraft.util.Mth;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;

final class SpritePlacement {
	private static final float SPRITE_BACK_TO_BLOCK_FRONT = 17.0F / 32.0F;
	private static final float SPRITE_CENTER_Z = -1.0F / 32.0F;
	private static final Matrix4fc MIRROR = new Matrix4f().scaling(-1.0F, 1.0F, 1.0F);
	private static final Matrix4fc ROTATED_TO_BLOCK = new Matrix4f()
		.translate(0.5F, 0.5F, 0.5F)
		.rotateY(radians(180.0F))
		.translate(-0.5F, -0.5F, -SPRITE_CENTER_Z);
	private static final Matrix4fc REFLECTED_TO_BLOCK = new Matrix4f().translate(1.0F, 0.0F, SPRITE_BACK_TO_BLOCK_FRONT).scale(-1.0F, 1.0F, 1.0F);
	static final Matrix4fc ITEM = new Matrix4f()
		.translate(0.0F, -0.3F, 0.0F)
		.scale(1.5F)
		.rotateY(radians(50.0F))
		.rotateZ(radians(335.0F))
		.translate(-0.9375F, -0.0625F, 0.0F);

	private SpritePlacement() {
	}

	static float radians(float degrees) {
		return degrees * Mth.DEG_TO_RAD;
	}

	static Matrix4f mirrored(Matrix4fc matrix) {
		return new Matrix4f(MIRROR).mul(matrix).mul(MIRROR);
	}

	static Matrix4f fit(Matrix4fc target, Matrix4fc display) {
		Matrix4fc toBlock = target.determinant() < 0.0F ? REFLECTED_TO_BLOCK : ROTATED_TO_BLOCK;
		Matrix4f applied = new Matrix4f(display).mul(toBlock);
		return new Matrix4f(target).mul(applied.invert());
	}
}
