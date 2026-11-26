package sh.rikka.client.mixin.scale;

import com.mojang.blaze3d.pipeline.RenderTarget;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.boost.scale.RenderScaleHooks;

@Mixin(GameRenderer.class)
abstract class GameRendererMixin {
	@Unique
	private static final String RENDER_LEVEL = "Lnet/minecraft/client/renderer/GameRenderer;renderLevel()V";

	@Unique
	private static final String APPLY_POST_EFFECTS = "Lnet/minecraft/client/renderer/GameRenderer;applyPostEffects()V";

	@Shadow
	@Final
	private Minecraft minecraft;

	@Shadow
	@Final
	private RenderTarget mainRenderTarget;

	@Shadow
	@Final
	private RenderTarget hud3DTarget;

	@Inject(method = "render", at = @At(value = "INVOKE", target = RENDER_LEVEL))
	private void rikka$scaledLevelStarting(CallbackInfo callback) {
		RenderScaleHooks.levelStarting(this.mainRenderTarget, this.hud3DTarget, this.rikka$level());
	}

	@Inject(method = "render", at = @At(value = "INVOKE", target = APPLY_POST_EFFECTS, shift = At.Shift.AFTER))
	private void rikka$scaledLevelFinished(CallbackInfo callback) {
		RenderScaleHooks.levelFinished(this.mainRenderTarget, this.hud3DTarget, this.rikka$level());
	}

	@Unique
	private LevelRendererAccessor rikka$level() {
		return (LevelRendererAccessor) this.minecraft.levelRenderer;
	}
}
