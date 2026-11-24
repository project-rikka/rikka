package sh.rikka.client.mods.motionblur;

import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;
import org.joml.Vector3f;
import org.jspecify.annotations.Nullable;

final class CameraMotion {
	private static final double CUT_DISTANCE = 16.0;

	private final Matrix4f previousView = new Matrix4f();
	private final Matrix4f previousProjection = new Matrix4f();
	private Vec3 previousPosition = Vec3.ZERO;
	private @Nullable ClientLevel previousLevel;
	private boolean tracking;

	CameraStep advance(CameraRenderState camera, Matrix4fc projection, @Nullable ClientLevel level, boolean continuous) {
		Matrix4fc view = camera.viewRotationMatrix;
		Vec3 position = camera.pos;
		boolean cut = !this.tracking
			|| !continuous
			|| level != this.previousLevel
			|| position.distanceToSqr(this.previousPosition) > CUT_DISTANCE * CUT_DISTANCE;
		CameraStep step = new CameraStep(
			view.invert(new Matrix4f()),
			projection.invert(new Matrix4f()),
			new Matrix4f(cut ? view : this.previousView),
			new Matrix4f(cut ? projection : this.previousProjection),
			cut ? new Vector3f() : position.subtract(this.previousPosition).toVector3f(),
			cut
		);
		this.previousView.set(view);
		this.previousProjection.set(projection);
		this.previousPosition = position;
		this.previousLevel = level;
		this.tracking = true;
		return step;
	}

	void reset() {
		this.tracking = false;
		this.previousLevel = null;
	}
}
