package sh.rikka.client.mods.animation;

import net.minecraft.util.Mth;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;

final class FirstPersonPose {
	private static final float PI = Mth.PI;
	private static final float BASE_X = 0.56F;
	private static final float BASE_Y = -0.52F;
	private static final float BASE_Z = -0.72F;
	private static final float HEIGHT_SCALE = -0.6F;
	private static final float ITEM_SCALE = 0.4F;
	private static final Matrix4fc VANILLA_FRAME = new Matrix4f().rotateY(SpritePlacement.radians(45.0F)).scale(ITEM_SCALE).invert();
	private static final float EAT_BOB_THRESHOLD = 0.8F;
	private static final float EAT_EXPONENT = 27.0F;
	private static final float BOW_FULL_DRAW_TICKS = 20.0F;

	private FirstPersonPose() {
	}

	enum Use {
		PLAIN,
		EAT,
		BOW,
		BLOCK
	}

	record Frame(
		Use use,
		boolean using,
		boolean swinging,
		float partialTicks,
		float inverseArmHeight,
		float swing,
		int remainingTicks,
		int useDuration
	) {
	}

	static Matrix4f chain(Frame frame) {
		Matrix4f chain = new Matrix4f();
		float swingRoot = Mth.sqrt(frame.swing());
		float swingLift = Mth.sin(swingRoot * PI);
		if (frame.use() == Use.EAT) {
			eat(chain, frame);
		} else if (!frame.using()) {
			chain.translate(-swingLift * 0.4F, Mth.sin(swingRoot * PI * 2.0F) * 0.2F, -Mth.sin(frame.swing() * PI) * 0.2F);
		}
		chain.translate(BASE_X, BASE_Y + frame.inverseArmHeight() * HEIGHT_SCALE, BASE_Z);
		chain.rotateY(SpritePlacement.radians(45.0F));
		if (frame.swinging()) {
			chain.rotateY(SpritePlacement.radians(-Mth.sin(frame.swing() * frame.swing() * PI) * 20.0F));
			chain.rotateZ(SpritePlacement.radians(-swingLift * 20.0F));
			chain.rotateX(SpritePlacement.radians(-swingLift * 80.0F));
		}
		chain.scale(ITEM_SCALE);
		switch (frame.use()) {
			case BLOCK -> block(chain);
			case BOW -> bow(chain, frame);
			case PLAIN, EAT -> {
			}
		}
		return chain;
	}

	static Matrix4f exact(Matrix4f chain, ItemFamily family, Matrix4f display) {
		Matrix4f target = new Matrix4f(chain);
		if (family.rotatesAround()) {
			target.rotateY(SpritePlacement.radians(180.0F));
		}
		target.mul(SpritePlacement.ITEM);
		return SpritePlacement.fit(target, display);
	}

	static Matrix4f approximate(Matrix4f chain) {
		return chain.mul(VANILLA_FRAME);
	}

	private static void eat(Matrix4f chain, Frame frame) {
		float remaining = frame.remainingTicks() - frame.partialTicks() + 1.0F;
		float scaled = remaining / frame.useDuration();
		if (scaled < EAT_BOB_THRESHOLD) {
			chain.translate(0.0F, Math.abs(Mth.cos(remaining / 4.0F * PI) * 0.1F), 0.0F);
		}
		float jiggle = 1.0F - (float) Math.pow(scaled, EAT_EXPONENT);
		chain.translate(jiggle * 0.6F, jiggle * -0.5F, 0.0F);
		chain.rotateY(SpritePlacement.radians(jiggle * 90.0F));
		chain.rotateX(SpritePlacement.radians(jiggle * 10.0F));
		chain.rotateZ(SpritePlacement.radians(jiggle * 30.0F));
	}

	private static void block(Matrix4f chain) {
		chain.translate(-0.5F, 0.2F, 0.0F);
		chain.rotateY(SpritePlacement.radians(30.0F));
		chain.rotateX(SpritePlacement.radians(-80.0F));
		chain.rotateY(SpritePlacement.radians(60.0F));
	}

	private static void bow(Matrix4f chain, Frame frame) {
		chain.rotateZ(SpritePlacement.radians(-18.0F));
		chain.rotateY(SpritePlacement.radians(-12.0F));
		chain.rotateX(SpritePlacement.radians(-8.0F));
		chain.translate(-0.9F, 0.2F, 0.0F);
		float timeHeld = frame.useDuration() - (frame.remainingTicks() - frame.partialTicks() + 1.0F);
		float power = timeHeld / BOW_FULL_DRAW_TICKS;
		power = Math.min((power * power + power * 2.0F) / 3.0F, 1.0F);
		if (power > 0.1F) {
			chain.translate(0.0F, Mth.sin((timeHeld - 0.1F) * 1.3F) * 0.01F * (power - 0.1F), 0.0F);
		}
		chain.translate(0.0F, 0.0F, power * 0.1F);
		chain.rotateZ(SpritePlacement.radians(-335.0F));
		chain.rotateY(SpritePlacement.radians(-50.0F));
		chain.translate(0.0F, 0.5F, 0.0F);
		chain.scale(1.0F, 1.0F, 1.0F + power * 0.2F);
		chain.translate(0.0F, -0.5F, 0.0F);
		chain.rotateY(SpritePlacement.radians(50.0F));
		chain.rotateZ(SpritePlacement.radians(335.0F));
	}
}
