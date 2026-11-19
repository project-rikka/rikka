package sh.rikka.client.ui;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import com.mojang.renderpearl.backend.vulkan.VulkanCommandEncoder;
import com.mojang.renderpearl.backend.vulkan.VulkanConst;
import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import com.mojang.renderpearl.backend.vulkan.VulkanFeatureSets;
import com.mojang.renderpearl.backend.vulkan.VulkanGpuTexture;
import com.mojang.renderpearl.backend.vulkan.VulkanGpuTextureView;
import com.mojang.renderpearl.backend.vulkan.VulkanQueue;
import com.mojang.renderpearl.backend.vulkan.init.FeatureSet;
import com.mojang.renderpearl.backend.vulkan.init.VulkanFeature;
import java.util.Set;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.vulkan.VK;
import org.lwjgl.vulkan.VK10;
import org.lwjgl.vulkan.VK13;
import org.lwjgl.vulkan.VkCommandBuffer;
import org.lwjgl.vulkan.VkDevice;
import org.lwjgl.vulkan.VkPhysicalDeviceFeatures2;
import os.rikka.VulkanImage;
import sh.rikka.client.mixin.VulkanDeviceAccessor;

public final class RikkaVulkan {
	public static final int API_VERSION = VK13.VK_API_VERSION_1_3;
	public static final FeatureSet FEATURES = new FeatureSet(
		"Rikka UI",
		Set.of(),
		Set.of(
			new VulkanFeature(VulkanFeatureSets.VK12_FEATURES_STRUCT, "timelineSemaphore"),
			new VulkanFeature(VulkanFeatureSets.VK12_FEATURES_STRUCT, "runtimeDescriptorArray"),
			new VulkanFeature(VulkanFeatureSets.VK12_FEATURES_STRUCT, "shaderSampledImageArrayNonUniformIndexing"),
			new VulkanFeature(VulkanFeatureSets.VK12_FEATURES_STRUCT, "descriptorBindingSampledImageUpdateAfterBind"),
			new VulkanFeature(VulkanFeatureSets.VK12_FEATURES_STRUCT, "descriptorBindingPartiallyBound"),
			new VulkanFeature(VulkanFeatureSets.VK12_FEATURES_STRUCT, "descriptorBindingUpdateUnusedWhilePending")
		)
	);

	private RikkaVulkan() {
	}

	static os.rikka.VulkanDevice describe(VulkanDevice device) {
		VkDevice vkDevice = device.vkDevice();
		VulkanQueue queue = device.graphicsQueue();
		VulkanCommandEncoder encoder = device.createCommandEncoder();
		os.rikka.VulkanDevice described = new os.rikka.VulkanDevice(
			VK.getFunctionProvider().getFunctionAddress("vkGetInstanceProcAddr"),
			device.instance().vkInstance().address(),
			API_VERSION,
			vkDevice.getPhysicalDevice().address(),
			vkDevice.address(),
			queue.vkQueue().address(),
			queue.queueFamilyIndex(),
			encoder::waitSemaphore,
			commandBuffer -> encoder.execute(new VkCommandBuffer(commandBuffer, vkDevice)),
			encoder::signalSemaphore
		);
		for (String extension : device.instance().getEnabledExtensions()) {
			described.addInstanceExtension(extension);
		}
		FeatureSet enabled = ((VulkanDeviceAccessor) device).rikka$enabledFeatures();
		for (String extension : enabled.extensions()) {
			described.addDeviceExtension(extension);
		}
		try (MemoryStack stack = MemoryStack.stackPush()) {
			VkPhysicalDeviceFeatures2 features = VkPhysicalDeviceFeatures2.calloc(stack).sType$Default();
			for (VulkanFeature feature : enabled.features()) {
				feature.set(features, true, stack);
			}
			described.features(features.address());
		}
		return described;
	}

	static VulkanImage image(RenderTarget target) {
		GpuTextureView view = target.getColorTextureView();
		if (!(view instanceof VulkanGpuTextureView color)) {
			throw new IllegalStateException("the render target has no Vulkan color texture");
		}
		VulkanGpuTexture texture = color.texture();
		return new VulkanImage(
			texture.vkImage(),
			color.vkImageView(),
			VulkanConst.toVk(texture.getFormat()),
			texture.getWidth(0),
			texture.getHeight(0),
			VK10.VK_IMAGE_LAYOUT_GENERAL,
			true
		);
	}
}
