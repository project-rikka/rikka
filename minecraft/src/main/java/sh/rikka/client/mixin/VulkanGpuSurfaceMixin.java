package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.sugar.Local;
import com.mojang.renderpearl.api.device.GpuSurface;
import com.mojang.renderpearl.backend.vulkan.VulkanGpuSurface;
import org.lwjgl.vulkan.VkSurfaceCapabilitiesKHR;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import sh.rikka.client.render.vulkan.SwapchainDepth;

@Mixin(VulkanGpuSurface.class)
abstract class VulkanGpuSurfaceMixin {
	@ModifyArg(
		method = "configure",
		at = @At(
			value = "INVOKE",
			target = "Lorg/lwjgl/vulkan/VkSwapchainCreateInfoKHR;minImageCount(I)Lorg/lwjgl/vulkan/VkSwapchainCreateInfoKHR;"
		)
	)
	private int rikka$imageCount(
		int requested,
		@Local(argsOnly = true) GpuSurface.Configuration configuration,
		@Local(name = "surfaceCapabilities") VkSurfaceCapabilitiesKHR capabilities
	) {
		return SwapchainDepth.imageCount(requested, capabilities.maxImageCount(), configuration.presentMode());
	}
}
