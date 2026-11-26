package sh.rikka.client.mixin.entity;

import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import java.util.List;
import net.minecraft.client.renderer.StagedVertexBuffer;
import net.minecraft.client.renderer.feature.FeatureFrameContext;
import net.minecraft.client.renderer.feature.ItemFeatureRenderer;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.render.skin.ItemSkin;
import sh.rikka.client.render.skin.SkinFrameHolder;
import sh.rikka.client.render.skin.SkinWriter;

@Mixin(ItemFeatureRenderer.class)
abstract class ItemFeatureRendererMixin {
	@Unique
	private final SkinWriter rikka$skinWriter = new SkinWriter();

	@Unique
	private @Nullable SkinFrameHolder rikka$skinTarget;

	@Inject(method = "buildGroup", at = @At("HEAD"))
	private void rikka$selectSkinTarget(FeatureFrameContext context, List<ItemFeatureRenderer.Submit> submits, CallbackInfo callback) {
		StagedVertexBuffer buffer = context.stagedVertexBuffer();
		this.rikka$skinTarget = RikkaClient.skins().isActive() ? (SkinFrameHolder) buffer : null;
	}

	@WrapMethod(method = "prepareMainSubmit")
	private void rikka$skinItem(ItemFeatureRenderer.Submit submit, Operation<Void> original) {
		SkinFrameHolder target = this.rikka$skinTarget;
		ItemStackRenderState.FoilType foilType = submit.foilType();
		if (target == null || foilType == ItemStackRenderState.FoilType.SPECIAL || !SkinWriter.isAffine(submit.pose())) {
			original.call(submit);
			return;
		}
		ItemSkin skin = RikkaClient.skins().item(submit.quads());
		if (!skin.isSupported()) {
			original.call(submit);
			return;
		}
		this.rikka$skinWriter.writeItem(
			target.rikka$skinFrame(),
			target.rikka$buildingGroup(),
			skin,
			foilType == ItemStackRenderState.FoilType.STANDARD,
			submit.pose(),
			submit.lightCoords(),
			submit.overlayCoords(),
			submit.tintLayers()
		);
	}
}
