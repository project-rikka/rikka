package sh.rikka.client.mixin;

import com.mojang.renderpearl.backend.vulkan.VulkanInstance;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import sh.rikka.client.ui.RikkaVulkan;

@Mixin(VulkanInstance.class)
abstract class VulkanInstanceMixin {
	@ModifyArg(
		method = "<init>",
		at = @At(value = "INVOKE", target = "Lorg/lwjgl/vulkan/VkApplicationInfo;apiVersion(I)Lorg/lwjgl/vulkan/VkApplicationInfo;")
	)
	private int rikka$apiVersion(int requested) {
		return Math.max(requested, RikkaVulkan.API_VERSION);
	}
}
