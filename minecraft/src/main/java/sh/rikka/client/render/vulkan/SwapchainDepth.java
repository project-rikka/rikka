package sh.rikka.client.render.vulkan;

import com.mojang.renderpearl.api.device.GpuSurface;

public final class SwapchainDepth {
	private static final int IMAGES_BEYOND_VANILLA = 2;
	private static final int UNLIMITED = 0;

	private SwapchainDepth() {
	}

	public static int imageCount(int vanilla, int surfaceMaximum, GpuSurface.PresentMode mode) {
		if (mode != GpuSurface.PresentMode.IMMEDIATE && mode != GpuSurface.PresentMode.MAILBOX) {
			return vanilla;
		}
		int wanted = vanilla + IMAGES_BEYOND_VANILLA;
		return surfaceMaximum == UNLIMITED ? wanted : Math.min(wanted, Math.max(vanilla, surfaceMaximum));
	}
}
