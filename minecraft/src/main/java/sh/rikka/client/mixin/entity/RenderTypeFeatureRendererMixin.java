package sh.rikka.client.mixin.entity;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import com.mojang.renderpearl.api.commands.RenderPass;
import net.minecraft.client.renderer.StagedVertexBuffer;
import net.minecraft.client.renderer.feature.RenderTypeFeatureRenderer;
import net.minecraft.client.renderer.oit.OitStage;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.render.skin.SkinFrame;
import sh.rikka.client.render.skin.SkinFrameHolder;

@Mixin(RenderTypeFeatureRenderer.class)
abstract class RenderTypeFeatureRendererMixin {
	@WrapOperation(
		method = "executeGroup",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/StagedVertexBuffer;getExecuteInfo(Lnet/minecraft/client/renderer/StagedVertexBuffer$Draw;)Lnet/minecraft/client/renderer/StagedVertexBuffer$ExecuteInfo;"
		)
	)
	private StagedVertexBuffer.@Nullable ExecuteInfo rikka$executeSkins(
		StagedVertexBuffer buffer,
		StagedVertexBuffer.Draw draw,
		Operation<StagedVertexBuffer.ExecuteInfo> original,
		@Local(argsOnly = true) @Nullable OitStage stage,
		@Local(argsOnly = true) RenderPass renderPass
	) {
		SkinFrame frame = ((SkinFrameHolder) buffer).rikka$openSkinFrame();
		if (frame != null && frame.execute(draw, renderPass, stage)) {
			return null;
		}
		return original.call(buffer, draw);
	}
}
