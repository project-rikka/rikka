package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import it.unimi.dsi.fastutil.objects.ObjectArrayList;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.SectionUpdateTracker;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.chunk.CompiledSectionMesh;
import net.minecraft.client.renderer.chunk.RenderRegionCache;
import net.minecraft.client.renderer.chunk.RenderSectionRegion;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.util.Util;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.mods.rearview.RearviewLevel;
import sh.rikka.client.mods.visual.VisualHooks;
import sh.rikka.client.render.terrain.CompileFeed;
import sh.rikka.client.render.terrain.DirtyStateEpoch;
import sh.rikka.client.render.terrain.TrackedSection;

@Mixin(LevelExtractor.class)
abstract class LevelExtractorMixin {
	@Shadow
	@Final
	private LevelRenderer levelRenderer;

	@Unique
	private long rikka$blockEntityMillis;

	@Unique
	private final CompileFeed rikka$compileFeed = new CompileFeed();

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

	@WrapOperation(
		method = "extract",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/SectionUpdateTracker;getDirtyState(J)Lnet/minecraft/client/SectionUpdateTracker$SectionDirtyState;"
		)
	)
	private SectionUpdateTracker.@Nullable SectionDirtyState rikka$dirtyState(
		SectionUpdateTracker tracker,
		long sectionNode,
		Operation<SectionUpdateTracker.SectionDirtyState> original,
		@Local(name = "section") SectionRenderDispatcher.RenderSection section
	) {
		TrackedSection tracked = (TrackedSection) section;
		int epoch = DirtyStateEpoch.current();
		if (!tracked.rikka$tracks(sectionNode, epoch)) {
			tracked.rikka$track(sectionNode, epoch, original.call(tracker, sectionNode));
		}
		SectionUpdateTracker.SectionDirtyState state = tracked.rikka$dirtyState();
		if (state != null && state.isDirty() && section.getSectionMesh() == CompiledSectionMesh.UNCOMPILED && !this.rikka$compileFeed.hasRoom()) {
			return null;
		}
		return state;
	}

	@Inject(method = "extract", at = @At(value = "NEW", target = "net/minecraft/client/renderer/chunk/RenderRegionCache"))
	private void rikka$openCompileFeed(DeltaTracker deltaTracker, Camera camera, float worldPartialTicks, CallbackInfo callback) {
		SectionRenderDispatcher dispatcher = this.levelRenderer.sectionRenderDispatcher();
		this.rikka$compileFeed.beginFrame(dispatcher == null ? 0 : dispatcher.getCompileQueueSize());
	}

	@WrapOperation(
		method = "extract",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/chunk/RenderRegionCache;createRegion(Lnet/minecraft/client/multiplayer/ClientLevel;J)Lnet/minecraft/client/renderer/chunk/RenderSectionRegion;"
		)
	)
	private RenderSectionRegion rikka$admitRegion(RenderRegionCache cache, ClientLevel level, long sectionNode, Operation<RenderSectionRegion> original) {
		this.rikka$compileFeed.admitted();
		return original.call(cache, level, sectionNode);
	}

	@Inject(method = "extractVisibleBlockEntities", at = @At("HEAD"))
	private void rikka$stampBlockEntities(CallbackInfo callback) {
		this.rikka$blockEntityMillis = Util.getMillis();
	}

	@WrapOperation(method = "extractVisibleBlockEntities", at = @At(value = "INVOKE", target = "Lnet/minecraft/util/Util;getMillis()J"))
	private long rikka$sharedBlockEntityMillis(Operation<Long> original) {
		return this.rikka$blockEntityMillis;
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
