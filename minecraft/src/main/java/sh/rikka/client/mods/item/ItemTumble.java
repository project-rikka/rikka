package sh.rikka.client.mods.item;

import net.minecraft.util.Mth;

public final class ItemTumble {
	private static final float DEGREES_PER_TICK = 20.0F;
	private static final float SUBMERGED_FACTOR = 0.25F;
	private static final float SETTLE_DEGREES_PER_TICK = 30.0F;
	private static final float MAX_STEP_TICKS = 2.0F;
	private static final float FULL_TURN = 360.0F;

	private float angle;
	private float lastAge = Float.NaN;

	float advance(float age, boolean resting, boolean submerged, float speed, float restStep) {
		float elapsed = Float.isNaN(this.lastAge) ? 0.0F : Mth.clamp(age - this.lastAge, 0.0F, MAX_STEP_TICKS);
		this.lastAge = age;
		if (resting) {
			float rest = Math.round(this.angle / restStep) * restStep;
			float step = SETTLE_DEGREES_PER_TICK * elapsed;
			this.angle = Math.abs(rest - this.angle) <= step ? rest : this.angle + Math.signum(rest - this.angle) * step;
		} else {
			this.angle += elapsed * DEGREES_PER_TICK * speed * (submerged ? SUBMERGED_FACTOR : 1.0F);
		}
		this.angle %= FULL_TURN;
		return this.angle;
	}
}
