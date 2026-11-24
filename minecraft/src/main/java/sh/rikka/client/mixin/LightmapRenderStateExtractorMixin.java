package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.visual.VisualHooks;

@Mixin(LightmapRenderStateExtractor.class)
abstract class LightmapRenderStateExtractorMixin {
	@ModifyExpressionValue(method = "extract", at = @At(value = "INVOKE", target = "Ljava/lang/Math;max(FF)F"))
	private float rikka$brightness(float vanilla) {
		return VisualHooks.lightmapBrightness(vanilla);
	}
}
