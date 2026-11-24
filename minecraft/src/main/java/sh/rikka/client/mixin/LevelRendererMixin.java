package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.commands.RenderPass;
import java.util.Optional;
import java.util.OptionalDouble;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.LevelTargetBundle;
import net.minecraft.client.renderer.chunk.ChunkSectionsToRender;
import net.minecraft.client.renderer.feature.FeatureRenderDispatcher;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.motionblur.MotionBlurHooks;

@Mixin(LevelRenderer.class)
abstract class LevelRendererMixin {
	private static final String SOLID_FEATURES =
		"Lnet/minecraft/client/renderer/feature/FeatureRenderDispatcher$PreparedFrame;executeSolid(Lcom/mojang/renderpearl/api/commands/RenderPass;)V";

	@Shadow
	@Final
	private LevelTargetBundle targets;

	@Shadow
	@Final
	private GameRenderer gameRenderer;

	@Unique
	private @Nullable RenderPass rikka$featurePass;

	@WrapOperation(method = "executeSolid", at = @At(value = "INVOKE", target = SOLID_FEATURES))
	private void rikka$blurTerrain(FeatureRenderDispatcher.PreparedFrame frame, RenderPass terrainPass, Operation<Void> original) {
		if (!MotionBlurHooks.blursTerrain()) {
			original.call(frame, terrainPass);
			return;
		}
		RenderTarget main = this.targets.main.get();
		terrainPass.close();
		MotionBlurHooks.terrainDrawn(main);
		RenderPass featurePass = RenderSystem.getDevice()
			.createCommandEncoder()
			.createRenderPass(() -> "Solid features", main.getColorTextureView(), Optional.empty(), main.getDepthTextureView(), OptionalDouble.empty());
		RenderSystem.bindDefaultUniforms(featurePass);
		try {
			original.call(frame, featurePass);
		} catch (RuntimeException failure) {
			featurePass.close();
			throw failure;
		}
		if (this.gameRenderer.useImprovedTransparency()) {
			featurePass.close();
		} else {
			this.rikka$featurePass = featurePass;
		}
	}

	@WrapMethod(method = "executeClassicTransparency")
	private void rikka$continueTransparency(
		ChunkSectionsToRender sections,
		FeatureRenderDispatcher.PreparedFrame frame,
		RenderPass pass,
		Operation<Void> original
	) {
		RenderPass featurePass = this.rikka$featurePass;
		if (featurePass == null) {
			original.call(sections, frame, pass);
			return;
		}
		this.rikka$featurePass = null;
		try (featurePass) {
			original.call(sections, frame, featurePass);
		}
	}
}
