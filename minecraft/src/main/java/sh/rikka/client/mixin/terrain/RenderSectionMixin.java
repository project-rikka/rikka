package sh.rikka.client.mixin.terrain;

import net.minecraft.client.SectionUpdateTracker;
import net.minecraft.client.renderer.chunk.SectionMesh;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import sh.rikka.client.render.terrain.TerrainRenderer;
import sh.rikka.client.render.terrain.TrackedSection;

@Mixin(SectionRenderDispatcher.RenderSection.class)
abstract class RenderSectionMixin implements TrackedSection {
	@Unique
	private long rikka$trackedNode;

	@Unique
	private int rikka$trackedEpoch = -1;

	@Unique
	private SectionUpdateTracker.@Nullable SectionDirtyState rikka$dirtyState;

	@Override
	public boolean rikka$tracks(long sectionNode, int epoch) {
		return this.rikka$trackedEpoch == epoch && this.rikka$trackedNode == sectionNode;
	}

	@Override
	public SectionUpdateTracker.@Nullable SectionDirtyState rikka$dirtyState() {
		return this.rikka$dirtyState;
	}

	@Override
	public void rikka$track(long sectionNode, int epoch, SectionUpdateTracker.@Nullable SectionDirtyState state) {
		this.rikka$trackedNode = sectionNode;
		this.rikka$trackedEpoch = epoch;
		this.rikka$dirtyState = state;
	}

	@Inject(method = "<init>", at = @At("TAIL"))
	private void rikka$created(CallbackInfo callback) {
		TerrainRenderer.sectionCreated((SectionRenderDispatcher.RenderSection) (Object) this);
	}

	@Inject(method = {"reset", "indexBufferUploadCallback"}, at = @At("TAIL"))
	private void rikka$changed(CallbackInfo callback) {
		TerrainRenderer.sectionChanged((SectionRenderDispatcher.RenderSection) (Object) this);
	}

	@Inject(method = "setSectionMesh", at = @At("RETURN"))
	private void rikka$meshChanged(CallbackInfoReturnable<SectionMesh> callback) {
		TerrainRenderer.sectionChanged((SectionRenderDispatcher.RenderSection) (Object) this);
	}
}
