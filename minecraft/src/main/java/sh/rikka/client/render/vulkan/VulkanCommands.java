package sh.rikka.client.render.vulkan;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.api.buffers.GpuBuffer;
import com.mojang.renderpearl.api.buffers.GpuBufferSlice;
import com.mojang.renderpearl.api.commands.RenderPass;
import com.mojang.renderpearl.backend.vulkan.VulkanCommandEncoder;
import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import com.mojang.renderpearl.backend.vulkan.VulkanGpuBuffer;
import java.nio.ByteBuffer;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.vulkan.VK10;
import org.lwjgl.vulkan.VkBufferCopy;
import org.lwjgl.vulkan.VkCommandBuffer;
import sh.rikka.client.mixin.FrontendGpuDeviceAccessor;
import sh.rikka.client.mixin.FrontendRenderPassAccessor;
import sh.rikka.client.mixin.VulkanCommandEncoderInvoker;
import sh.rikka.client.mixin.VulkanRenderPassAccessor;

public final class VulkanCommands {
	private static final long STAGING_ALIGNMENT = 4L;

	private VulkanCommands() {
	}

	public static VulkanDevice device() {
		return (VulkanDevice) ((FrontendGpuDeviceAccessor) RenderSystem.getDevice()).rikka$backend();
	}

	public static VkCommandBuffer outsidePass() {
		return ((VulkanCommandEncoderInvoker) device().createCommandEncoder()).rikka$commandBuffer();
	}

	public static VulkanRenderPassAccessor inside(RenderPass pass) {
		return (VulkanRenderPassAccessor) ((FrontendRenderPassAccessor) pass).rikka$backend();
	}

	public static long handle(GpuBuffer buffer) {
		return ((VulkanGpuBuffer) buffer).vkBuffer();
	}

	public static void barrier(VkCommandBuffer commands) {
		try (MemoryStack stack = MemoryStack.stackPush()) {
			VulkanCommandEncoder.memoryBarrier(commands, stack);
		}
	}

	public static Staged stage(long bytes) {
		GpuBufferSlice.MappedView view = device().createCommandEncoder().transientMemory().allocateStaging(bytes, STAGING_ALIGNMENT, GpuBuffer.USAGE_COPY_SRC);
		return new Staged(view);
	}

	public static final class Staged implements AutoCloseable {
		private final GpuBufferSlice.MappedView view;

		private Staged(GpuBufferSlice.MappedView view) {
			this.view = view;
		}

		public ByteBuffer data() {
			return this.view.data();
		}

		public void copyTo(VkCommandBuffer commands, DeviceBuffer target, VkBufferCopy.Buffer regions) {
			GpuBufferSlice slice = this.view.slice();
			for (int region = regions.position(); region < regions.limit(); region++) {
				regions.get(region).srcOffset(regions.get(region).srcOffset() + slice.offset());
			}
			VK10.vkCmdCopyBuffer(commands, handle(slice.buffer()), target.handle(), regions);
		}

		public void copyTo(VkCommandBuffer commands, DeviceBuffer target, long targetOffset, long bytes) {
			try (MemoryStack stack = MemoryStack.stackPush()) {
				this.copyTo(commands, target, VkBufferCopy.calloc(1, stack).srcOffset(0L).dstOffset(targetOffset).size(bytes));
			}
		}

		@Override
		public void close() {
			this.view.close();
		}
	}
}
