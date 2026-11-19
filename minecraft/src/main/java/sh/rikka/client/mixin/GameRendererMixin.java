package sh.rikka.client.mixin;

import com.mojang.blaze3d.pipeline.RenderTarget;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;

@Mixin(GameRenderer.class)
abstract class GameRendererMixin {
	private static final String GUI_RENDER = "Lnet/minecraft/client/gui/render/GuiRenderer;render()V";

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
}
