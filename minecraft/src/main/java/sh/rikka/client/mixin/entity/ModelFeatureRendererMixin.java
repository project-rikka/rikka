package sh.rikka.client.mixin.entity;

import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import java.util.List;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.StagedVertexBuffer;
import net.minecraft.client.renderer.feature.FeatureFrameContext;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.rendertype.RenderType;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.render.skin.ModelSkin;
import sh.rikka.client.render.skin.SkinFrameHolder;
import sh.rikka.client.render.skin.SkinWriter;
import sh.rikka.client.render.skin.Skins;
import sh.rikka.client.render.skin.UvTransform;

@Mixin(ModelFeatureRenderer.class)
abstract class ModelFeatureRendererMixin {
	@Unique
	private final SkinWriter rikka$skinWriter = new SkinWriter();

	@Unique
	private @Nullable SkinFrameHolder rikka$skinTarget;

	@Inject(method = "buildGroup", at = @At("HEAD"))
	private void rikka$selectSkinTarget(FeatureFrameContext context, List<ModelFeatureRenderer.Submit<?>> submits, CallbackInfo callback) {
		StagedVertexBuffer buffer = context.stagedVertexBuffer();
		this.rikka$skinTarget = RikkaClient.skins().isActive() ? (SkinFrameHolder) buffer : null;
	}

	@WrapMethod(method = "prepareModel")
	private <S> void rikka$skinModel(ModelFeatureRenderer.Submit<S> submit, Operation<Void> original) {
		SkinFrameHolder target = this.rikka$skinTarget;
		if (target == null || !this.rikka$writeSkin(target, submit)) {
			original.call(submit);
		}
	}

	@Unique
	private <S> boolean rikka$writeSkin(SkinFrameHolder target, ModelFeatureRenderer.Submit<S> submit) {
		Skins skins = RikkaClient.skins();
		RenderType renderType = submit.renderType();
		Model<? super S> model = submit.model();
		if (submit.sheetedDecalPose() != null || !SkinWriter.isAffine(submit.pose()) || !skins.pipelines().supports(renderType.pipeline())) {
			return false;
		}
		ModelSkin skin = skins.model(model.root());
		if (!skin.isSupported()) {
			return false;
		}
		model.setupAnim(submit.state());
		this.rikka$skinWriter.writeModel(
			target.rikka$skinFrame(),
			target.rikka$buildingGroup(),
			renderType,
			skin,
			model.root(),
			submit.pose(),
			submit.lightCoords(),
			submit.overlayCoords(),
			submit.tintedColor(),
			UvTransform.of(submit.uvMapping())
		);
		return true;
	}
}
