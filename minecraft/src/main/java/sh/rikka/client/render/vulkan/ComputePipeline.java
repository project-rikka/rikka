package sh.rikka.client.render.vulkan;

import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.ByteBuffer;
import java.nio.LongBuffer;
import java.nio.charset.StandardCharsets;
import net.minecraft.client.renderer.ShaderDefines;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.util.shaderc.Shaderc;
import org.lwjgl.vulkan.KHRPushDescriptor;
import org.lwjgl.vulkan.VK10;
import org.lwjgl.vulkan.VkCommandBuffer;
import org.lwjgl.vulkan.VkComputePipelineCreateInfo;
import org.lwjgl.vulkan.VkDescriptorSetLayoutBinding;
import org.lwjgl.vulkan.VkDescriptorSetLayoutCreateInfo;
import org.lwjgl.vulkan.VkDevice;
import org.lwjgl.vulkan.VkPipelineLayoutCreateInfo;
import org.lwjgl.vulkan.VkPipelineShaderStageCreateInfo;
import org.lwjgl.vulkan.VkPushConstantRange;
import org.lwjgl.vulkan.VkShaderModuleCreateInfo;
import org.lwjgl.vulkan.VkWriteDescriptorSet;

public final class ComputePipeline implements AutoCloseable {
	private static final String ENTRY_POINT = "main";

	private final VulkanDevice device;
	private final long setLayout;
	private final long layout;
	private final long pipeline;

	public ComputePipeline(VulkanDevice device, Class<?> owner, String resource, ShaderDefines defines, int[] descriptorTypes, int pushConstantBytes) {
		this.device = device;
		VkDevice vk = device.vkDevice();
		try (MemoryStack stack = MemoryStack.stackPush()) {
			VkDescriptorSetLayoutBinding.Buffer bindings = VkDescriptorSetLayoutBinding.calloc(descriptorTypes.length, stack);
			for (int binding = 0; binding < descriptorTypes.length; binding++) {
				bindings.get(binding)
					.binding(binding)
					.descriptorType(descriptorTypes[binding])
					.descriptorCount(1)
					.stageFlags(VK10.VK_SHADER_STAGE_COMPUTE_BIT);
			}
			LongBuffer handle = stack.callocLong(1);
			VkDescriptorSetLayoutCreateInfo setInfo = VkDescriptorSetLayoutCreateInfo.calloc(stack)
				.sType$Default()
				.flags(KHRPushDescriptor.VK_DESCRIPTOR_SET_LAYOUT_CREATE_PUSH_DESCRIPTOR_BIT_KHR)
				.pBindings(bindings);
			check(VK10.vkCreateDescriptorSetLayout(vk, setInfo, null, handle), resource);
			this.setLayout = handle.get(0);
			VkPipelineLayoutCreateInfo layoutInfo = VkPipelineLayoutCreateInfo.calloc(stack).sType$Default().pSetLayouts(stack.longs(this.setLayout));
			if (pushConstantBytes > 0) {
				layoutInfo.pPushConstantRanges(
					VkPushConstantRange.calloc(1, stack).stageFlags(VK10.VK_SHADER_STAGE_COMPUTE_BIT).offset(0).size(pushConstantBytes)
				);
			}
			check(VK10.vkCreatePipelineLayout(vk, layoutInfo, null, handle), resource);
			this.layout = handle.get(0);
			long module = compile(vk, owner, resource, defines, stack);
			VkComputePipelineCreateInfo.Buffer pipelineInfo = VkComputePipelineCreateInfo.calloc(1, stack).sType$Default().layout(this.layout);
			VkPipelineShaderStageCreateInfo stage = pipelineInfo.stage();
			stage.sType$Default().stage(VK10.VK_SHADER_STAGE_COMPUTE_BIT).module(module).pName(stack.UTF8(ENTRY_POINT));
			int created = VK10.vkCreateComputePipelines(vk, VK10.VK_NULL_HANDLE, pipelineInfo, null, handle);
			VK10.vkDestroyShaderModule(vk, module, null);
			check(created, resource);
			this.pipeline = handle.get(0);
		}
	}

	public void bind(VkCommandBuffer commands) {
		VK10.vkCmdBindPipeline(commands, VK10.VK_PIPELINE_BIND_POINT_COMPUTE, this.pipeline);
	}

	public void pushDescriptors(VkCommandBuffer commands, VkWriteDescriptorSet.Buffer writes) {
		KHRPushDescriptor.vkCmdPushDescriptorSetKHR(commands, VK10.VK_PIPELINE_BIND_POINT_COMPUTE, this.layout, 0, writes);
	}

	public void pushConstants(VkCommandBuffer commands, ByteBuffer constants) {
		VK10.vkCmdPushConstants(commands, this.layout, VK10.VK_SHADER_STAGE_COMPUTE_BIT, 0, constants);
	}

	@Override
	public void close() {
		VkDevice vk = this.device.vkDevice();
		long pipeline = this.pipeline;
		long layout = this.layout;
		long setLayout = this.setLayout;
		this.device.createCommandEncoder().queueForDestroy(() -> {
			VK10.vkDestroyPipeline(vk, pipeline, null);
			VK10.vkDestroyPipelineLayout(vk, layout, null);
			VK10.vkDestroyDescriptorSetLayout(vk, setLayout, null);
		});
	}

	private static long compile(VkDevice vk, Class<?> owner, String resource, ShaderDefines defines, MemoryStack stack) {
		String source;
		try (InputStream stream = owner.getResourceAsStream(resource)) {
			if (stream == null) {
				throw new IllegalStateException("missing shader " + resource);
			}
			source = new String(stream.readAllBytes(), StandardCharsets.UTF_8);
		} catch (IOException error) {
			throw new UncheckedIOException("failed to read shader " + resource, error);
		}
		long compiler = Shaderc.shaderc_compiler_initialize();
		long options = Shaderc.shaderc_compile_options_initialize();
		Shaderc.shaderc_compile_options_set_target_env(options, Shaderc.shaderc_target_env_vulkan, Shaderc.shaderc_env_version_vulkan_1_3);
		Shaderc.shaderc_compile_options_set_optimization_level(options, Shaderc.shaderc_optimization_level_performance);
		defines.values().forEach((name, value) -> Shaderc.shaderc_compile_options_add_macro_definition(options, name, value));
		defines.flags().forEach(flag -> Shaderc.shaderc_compile_options_add_macro_definition(options, flag, null));
		long result = Shaderc.shaderc_compile_into_spv(compiler, source, Shaderc.shaderc_compute_shader, resource, ENTRY_POINT, options);
		try {
			if (Shaderc.shaderc_result_get_compilation_status(result) != Shaderc.shaderc_compilation_status_success) {
				throw new IllegalStateException("failed to compile shader " + resource + ": " + Shaderc.shaderc_result_get_error_message(result));
			}
			ByteBuffer code = Shaderc.shaderc_result_get_bytes(result);
			LongBuffer module = stack.callocLong(1);
			check(VK10.vkCreateShaderModule(vk, VkShaderModuleCreateInfo.calloc(stack).sType$Default().pCode(code), null, module), resource);
			return module.get(0);
		} finally {
			Shaderc.shaderc_result_release(result);
			Shaderc.shaderc_compile_options_release(options);
			Shaderc.shaderc_compiler_release(compiler);
		}
	}

	private static void check(int result, String resource) {
		if (result != VK10.VK_SUCCESS) {
			throw new IllegalStateException("failed to create the pipeline of " + resource + ": " + result);
		}
	}
}
