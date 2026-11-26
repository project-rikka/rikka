package sh.rikka.client.mixin.entity;

import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import it.unimi.dsi.fastutil.objects.Reference2ReferenceOpenHashMap;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.StagedVertexBuffer;
import net.minecraft.client.renderer.rendertype.PreparedRenderType;
import net.minecraft.client.renderer.rendertype.RenderType;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.render.skin.SkinBatch;
import sh.rikka.client.render.skin.SkinFaces;
import sh.rikka.client.render.skin.SkinFrame;
import sh.rikka.client.render.skin.SkinFrameHolder;
import sh.rikka.client.render.skin.SkinGroup;

@Mixin(targets = "net.minecraft.client.renderer.feature.RenderTypeFeatureRenderer$Group")
abstract class RenderTypeGroupMixin implements SkinGroup {
	@Unique
	private static final String SKIN_SUFFIX = " (skin)";

	@Shadow
	@Final
	private StagedVertexBuffer stagedBuffer;

	@Shadow
	@Final
	private boolean canReorder;

	@Shadow
	@Final
	private List<StagedVertexBuffer.Draw> draws;

	@Shadow
	@Final
	private List<PreparedRenderType> drawRenderTypes;

	@Shadow
	private @Nullable RenderType lastRenderType;

	@Shadow
	private StagedVertexBuffer.@Nullable Draw lastDraw;

	@Unique
	private final Reference2ReferenceOpenHashMap<RenderType, StagedVertexBuffer.Draw> rikka$consolidatedDraws = new Reference2ReferenceOpenHashMap<>();

	@Unique
	private final Reference2ReferenceOpenHashMap<RenderType, SkinBatch> rikka$consolidatedSkins = new Reference2ReferenceOpenHashMap<>();

	@Unique
	private @Nullable SkinBatch rikka$lastSkin;

	@Unique
	private @Nullable RenderType rikka$lastSkinType;

	@Unique
	private StagedVertexBuffer.@Nullable Draw rikka$lastSkinDraw;

	@Inject(method = "<init>", at = @At("RETURN"))
	private void rikka$announce(StagedVertexBuffer stagedBuffer, boolean canReorder, CallbackInfo callback) {
		((SkinFrameHolder) stagedBuffer).rikka$setBuildingGroup(this);
	}

	@WrapMethod(method = "getOrAddDraw")
	private StagedVertexBuffer.Draw rikka$consolidateOnce(RenderType renderType, Operation<StagedVertexBuffer.Draw> original) {
		if (!this.canReorder || !renderType.canConsolidateConsecutiveGeometry()) {
			return original.call(renderType);
		}
		StagedVertexBuffer.Draw draw = this.rikka$consolidatedDraws.get(renderType);
		if (draw == null) {
			draw = original.call(renderType);
			this.rikka$consolidatedDraws.put(renderType, draw);
		}
		return draw;
	}

	@Override
	public SkinBatch rikka$skinBatch(SkinFrame frame, RenderType renderType) {
		SkinBatch last = this.rikka$lastSkin;
		if (last != null && this.rikka$lastSkinType == renderType && (this.canReorder || this.draws.getLast() == this.rikka$lastSkinDraw)) {
			return last;
		}
		SkinBatch batch = this.canReorder ? this.rikka$consolidatedSkins.get(renderType) : null;
		if (batch == null) {
			batch = this.rikka$addSkinBatch(frame, renderType);
			this.rikka$lastSkinDraw = this.draws.getLast();
			if (this.canReorder) {
				this.rikka$consolidatedSkins.put(renderType, batch);
			}
		}
		this.rikka$lastSkin = batch;
		this.rikka$lastSkinType = renderType;
		return batch;
	}

	@Unique
	private SkinBatch rikka$addSkinBatch(SkinFrame frame, RenderType renderType) {
		PreparedRenderType prepared = renderType.prepare();
		StagedVertexBuffer.Draw placeholder = this.stagedBuffer.appendDraw(renderType.format(), renderType.primitiveTopology());
		this.draws.add(placeholder);
		this.drawRenderTypes.add(new PreparedRenderType(
			prepared.name() + SKIN_SUFFIX,
			prepared.pipeline(),
			prepared.oitPipelineSet(),
			prepared.dynamicTransforms(),
			prepared.scissorState(),
			prepared.textures()
		));
		this.lastDraw = null;
		this.lastRenderType = null;
		boolean layered = renderType.sortOnUpload()
			&& SkinFaces.isLayered(renderType.pipeline())
			&& !Minecraft.getInstance().gameRenderer.useImprovedTransparency();
		return frame.createBatch(prepared, placeholder, !this.canReorder, layered);
	}
}
