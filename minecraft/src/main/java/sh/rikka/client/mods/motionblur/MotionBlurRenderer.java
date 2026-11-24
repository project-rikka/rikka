package sh.rikka.client.mods.motionblur;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.platform.Window;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import org.joml.Matrix4fc;
import org.jspecify.annotations.Nullable;

final class MotionBlurRenderer {
	private final FrameClock clock = new FrameClock();
	private final RefreshRate refreshRate = new RefreshRate();
	private final CameraMotion camera = new CameraMotion();
	private final VelocityBlur velocity = new VelocityBlur();
	private final FrameBlending blending = new FrameBlending();
	private final Accumulation accumulation = new Accumulation();
	private @Nullable FramePlan plan;

	void begin(MotionBlurMod mod, View view) {
		this.plan = null;
		if (view.camera().isPanoramicMode) {
			restart();
			return;
		}
		this.clock.tick(System.nanoTime());
		CameraStep step = this.camera.advance(view.camera(), view.projection(), view.level(), this.clock.continuous());
		if (step.cut()) {
			this.blending.restart();
			this.accumulation.restart();
		}
		float strength = mod.strength();
		BlurAlgorithm algorithm = strength > 0.0F ? mod.algorithm() : null;
		keepOnly(algorithm);
		if (algorithm == null) {
			return;
		}
		float refreshHz = this.refreshRate.hz(view.window());
		this.plan = new FramePlan(algorithm, strength, refreshHz, velocityPlan(mod, refreshHz, view, step));
	}

	boolean blursTerrain() {
		return this.plan != null && this.plan.velocity() != null;
	}

	void terrainDrawn(RenderTarget main) {
		VelocityPlan velocity = this.plan == null ? null : this.plan.velocity();
		if (velocity != null) {
			this.velocity.apply(velocity.terrain(), velocity.uniforms(), main);
		}
	}

	void levelDrawn(RenderTarget main) {
		VelocityPlan velocity = this.plan == null ? null : this.plan.velocity();
		if (velocity != null && velocity.rotation()) {
			this.velocity.apply(VelocityComponent.ROTATION, velocity.uniforms(), main);
		}
	}

	void finish(RenderTarget main) {
		FramePlan plan = this.plan;
		this.plan = null;
		if (plan == null) {
			return;
		}
		if (plan.algorithm().blendsFrames()) {
			this.blending.apply(plan.strength(), this.clock.smoothedFps(), plan.refreshHz(), this.clock.seconds(), main);
		}
		if (plan.algorithm().accumulates()) {
			this.accumulation.apply(plan.algorithm(), plan.strength(), main);
		}
		this.velocity.endFrame();
		this.blending.endFrame();
		this.accumulation.endFrame();
	}

	void refreshRateChanged() {
		this.refreshRate.invalidate();
	}

	void release() {
		this.plan = null;
		keepOnly(null);
		this.camera.reset();
		this.clock.reset();
	}

	private void restart() {
		this.camera.reset();
		this.clock.reset();
		this.blending.restart();
		this.accumulation.restart();
	}

	private @Nullable VelocityPlan velocityPlan(MotionBlurMod mod, float refreshHz, View view, CameraStep step) {
		BlurAlgorithm algorithm = mod.algorithm();
		if (!algorithm.usesVelocity()) {
			return null;
		}
		VelocityStrength velocityStrength = algorithm == BlurAlgorithm.HYBRID
			? VelocityStrength.betweenBlendedFrames(mod.strength(), this.clock.fps(), refreshHz)
			: VelocityStrength.of(mod.strength(), this.clock.fps(), refreshHz, mod.scalesWithRefreshRate());
		VelocityUniforms uniforms = new VelocityUniforms(step, velocityStrength, mod.profile());
		boolean followsCamera = !view.camera().isFirstPerson || view.riding();
		return new VelocityPlan(followsCamera ? VelocityComponent.FULL : VelocityComponent.TRANSLATION, !followsCamera, uniforms);
	}

	private void keepOnly(@Nullable BlurAlgorithm algorithm) {
		if (algorithm == null || !algorithm.usesVelocity()) {
			this.velocity.close();
		}
		if (algorithm == null || !algorithm.blendsFrames()) {
			this.blending.close();
		}
		if (algorithm == null || !algorithm.accumulates()) {
			this.accumulation.close();
		}
	}

	record View(CameraRenderState camera, Matrix4fc projection, @Nullable ClientLevel level, Window window, boolean riding) {
	}

	private record FramePlan(BlurAlgorithm algorithm, float strength, float refreshHz, @Nullable VelocityPlan velocity) {
	}

	private record VelocityPlan(VelocityComponent terrain, boolean rotation, VelocityUniforms uniforms) {
	}
}
