package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.Hud;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.events.ClientHooks;
import sh.rikka.client.mods.animation.AnimationHooks;
import sh.rikka.client.mods.visual.VisualHooks;

@Mixin(Hud.class)
abstract class HudMixin {
	@Inject(
		method = "extractRenderState",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/gui/Hud;extractChat(Lnet/minecraft/client/gui/GuiGraphicsExtractor;Lnet/minecraft/client/DeltaTracker;)V"
		)
	)
	private void rikka$extractHud(GuiGraphicsExtractor graphics, DeltaTracker deltaTracker, CallbackInfo callback) {
		ClientHooks.hudRender(graphics, deltaTracker);
	}

	@ModifyVariable(method = "extractHearts", at = @At("HEAD"), argsOnly = true)
	private boolean rikka$heartBlink(boolean blink) {
		return AnimationHooks.heartBlink(blink);
	}

	@ModifyExpressionValue(
		method = "extractCrosshair",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/client/CameraType;isFirstPerson()Z")
	)
	private boolean rikka$crosshairShown(boolean firstPerson) {
		return VisualHooks.crosshairShown(firstPerson);
	}

	@WrapOperation(
		method = "extractCrosshair",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIII)V",
			ordinal = 0
		)
	)
	private void rikka$crosshair(
		GuiGraphicsExtractor graphics, RenderPipeline pipeline, Identifier sprite, int x, int y, int width, int height, Operation<Void> original
	) {
		if (!VisualHooks.drawCrosshair(graphics)) {
			original.call(graphics, pipeline, sprite, x, y, width, height);
		}
	}
}
