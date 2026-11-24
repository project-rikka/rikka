package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.sugar.Local;
import com.mojang.blaze3d.pipeline.RenderTarget;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import org.joml.Matrix4f;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;
import sh.rikka.client.mods.motionblur.MotionBlurHooks;

@Mixin(GameRenderer.class)
abstract class GameRendererMixin {
	private static final String GUI_RENDER = "Lnet/minecraft/client/gui/render/GuiRenderer;render()V";
	private static final String LEVEL_RENDER =
		"Lnet/minecraft/client/renderer/LevelRenderer;render(Lcom/mojang/blaze3d/resource/GraphicsResourceAllocator;ZLnet/minecraft/client/renderer/state/level/CameraRenderState;Lcom/mojang/renderpearl/api/buffers/GpuBufferSlice;Lorg/joml/Vector4f;ZZ)V";

	@Shadow
	@Final
	private RenderTarget mainRenderTarget;

	@Inject(method = "render", at = @At(value = "INVOKE", target = GUI_RENDER))
	private void rikka$beneathGui(CallbackInfo callback) {
		ClientHooks.drawHud(this.mainRenderTarget);
	}

	@Inject(method = "render", at = @At(value = "INVOKE", target = GUI_RENDER, shift = At.Shift.AFTER))
	private void rikka$aboveGui(CallbackInfo callback) {
		ClientHooks.drawScreen(this.mainRenderTarget);
	}

	@Inject(method = "renderLevel", at = @At(value = "INVOKE", target = LEVEL_RENDER))
	private void rikka$levelStarting(
		CallbackInfo callback,
		@Local(name = "cameraState") CameraRenderState cameraState,
		@Local(name = "projectionMatrix") Matrix4f projectionMatrix
	) {
		MotionBlurHooks.levelStarting(cameraState, projectionMatrix);
	}

	@Inject(method = "renderLevel", at = @At(value = "INVOKE", target = LEVEL_RENDER, shift = At.Shift.AFTER))
	private void rikka$levelDrawn(CallbackInfo callback) {
		MotionBlurHooks.levelDrawn(this.mainRenderTarget);
	}

	@Inject(method = "renderLevel", at = @At("TAIL"))
	private void rikka$levelFinished(CallbackInfo callback) {
		MotionBlurHooks.levelFinished(this.mainRenderTarget);
	}
}
