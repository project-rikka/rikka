package sh.rikka.client.mixin.terrain;

import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import com.mojang.blaze3d.vertex.UberGpuBuffer;
import org.objectweb.asm.Opcodes;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.boost.BoostFeature;
import sh.rikka.client.render.terrain.CompactVertices;

@Mixin(UberGpuBuffer.class)
abstract class UberGpuBufferMixin {
	@ModifyExpressionValue(
		method = "uploadStagedAllocations",
		at = @At(value = "FIELD", target = "Lcom/mojang/blaze3d/vertex/UberGpuBuffer;alignSize:I", opcode = Opcodes.GETFIELD)
	)
	private int rikka$alignForCompactVertices(int alignment) {
		return BoostFeature.COMPACT_VERTICES.isActive() ? CompactVertices.alignment(alignment) : alignment;
	}
}
