package sh.rikka.client.render.vulkan;

import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import java.nio.LongBuffer;
import org.lwjgl.PointerBuffer;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.util.vma.Vma;
import org.lwjgl.util.vma.VmaAllocationCreateInfo;
import org.lwjgl.vulkan.VK10;
import org.lwjgl.vulkan.VkBufferCreateInfo;

public final class DeviceBuffer implements AutoCloseable {
	private final VulkanDevice device;
	private final long handle;
	private final long allocation;
	private final long size;

	public DeviceBuffer(VulkanDevice device, long size, int usage) {
		this.device = device;
		this.size = size;
		try (MemoryStack stack = MemoryStack.stackPush()) {
			VkBufferCreateInfo buffer = VkBufferCreateInfo.calloc(stack).sType$Default().size(size).usage(usage).sharingMode(VK10.VK_SHARING_MODE_EXCLUSIVE);
			VmaAllocationCreateInfo memory = VmaAllocationCreateInfo.calloc(stack).usage(Vma.VMA_MEMORY_USAGE_AUTO_PREFER_DEVICE);
			LongBuffer handle = stack.callocLong(1);
			PointerBuffer allocation = stack.callocPointer(1);
			int result = Vma.vmaCreateBuffer(device.vma(), buffer, memory, handle, allocation, null);
			if (result != VK10.VK_SUCCESS) {
				throw new IllegalStateException("failed to allocate a " + size + " byte buffer: " + result);
			}
			this.handle = handle.get(0);
			this.allocation = allocation.get(0);
		}
	}

	public long handle() {
		return this.handle;
	}

	public long size() {
		return this.size;
	}

	@Override
	public void close() {
		VulkanDevice device = this.device;
		long handle = this.handle;
		long allocation = this.allocation;
		device.createCommandEncoder().queueForDestroy(() -> Vma.vmaDestroyBuffer(device.vma(), handle, allocation));
	}
}
