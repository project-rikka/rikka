package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.extract.LevelExtractor;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.rearview.RearviewLevel;
import sh.rikka.client.mods.visual.VisualHooks;

@Mixin(LevelExtractor.class)
abstract class LevelExtractorMixin {
	@Shadow
	@Final
	private LevelRenderer levelRenderer;

	@ModifyExpressionValue(
		method = "extract",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/LevelRenderer;visibleSections()Lit/unimi/dsi/fastutil/objects/ObjectArrayList;"
		)
	)
	private ObjectArrayList<SectionRenderDispatcher.RenderSection> rikka$sectionsToUpdate(ObjectArrayList<SectionRenderDispatcher.RenderSection> visible) {
		return ((RearviewLevel) this.levelRenderer).rikka$sectionsToUpdate(visible);
	}

	@Inject(
		method = "extract",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/debug/DebugRenderer;emitGizmos(Lnet/minecraft/client/renderer/culling/Frustum;DDDF)V",
			shift = At.Shift.AFTER
		)
	)
	private void rikka$emitGizmos(DeltaTracker deltaTracker, Camera camera, float worldPartialTicks, CallbackInfo callback) {
		VisualHooks.emitGizmos(camera);
	}
}
