package sh.rikka.client.mixin.scale;

import com.mojang.renderpearl.backend.vulkan.VulkanGpuSampler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import sh.rikka.client.mods.boost.scale.TerrainSampler;

@Mixin(VulkanGpuSampler.class)
abstract class VulkanGpuSamplerMixin {
	@ModifyArg(
		method = "<init>",
		at = @At(value = "INVOKE", target = "Lorg/lwjgl/vulkan/VkSamplerCreateInfo;mipLodBias(F)Lorg/lwjgl/vulkan/VkSamplerCreateInfo;")
	)
	private float rikka$mipLodBias(float bias) {
		return bias + TerrainSampler.creatingBias();
	}
}
