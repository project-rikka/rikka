package sh.rikka.client.ui;

import com.mojang.blaze3d.pipeline.RenderTarget;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.renderpearl.backend.vulkan.VulkanDevice;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Set;
import net.minecraft.client.Minecraft;
import org.jspecify.annotations.Nullable;
import os.rikka.EmbeddedSurface;
import os.rikka.EmbeddedSurfaceBuilder;
import os.rikka.Embedding;
import os.rikka.VulkanImage;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.mixin.FrontendGpuDeviceAccessor;

public final class RikkaUi {
	private static @Nullable RikkaUi current;

	private final Embedding embedding;
	private final Set<EmbeddedSurface> surfaces = Collections.newSetFromMap(new IdentityHashMap<>());
	private final HudLayer hud;

	private RikkaUi(Embedding embedding) {
		this.embedding = embedding;
		this.hud = new HudLayer(this::open, RikkaClient.mods());
		RikkaClient.events().subscribe(this.hud);
	}

	public static void drawHud(RenderTarget target) {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.level == null || minecraft.gui.hud.isHidden()) {
			return;
		}
		RikkaUi ui = get();
		if (!ui.hud.isShowing()) {
			return;
		}
		VulkanImage image = RikkaVulkan.image(target);
		ui.hud.draw(image, SurfaceGeometry.of(minecraft.getWindow(), image));
	}

	public static void drawScreen(RenderTarget target) {
		Minecraft minecraft = Minecraft.getInstance();
		if (!(minecraft.gui.screen() instanceof RikkaScreen screen)) {
			return;
		}
		VulkanImage image = RikkaVulkan.image(target);
		screen.draw(image, SurfaceGeometry.of(minecraft.getWindow(), image));
	}

	static EmbeddedSurface openSurface(EmbeddedSurfaceBuilder surface) {
		return get().open(surface);
	}

	static void closeSurface(EmbeddedSurface surface) {
		RikkaUi ui = current;
		if (ui != null) {
			ui.surfaces.remove(surface);
		}
		surface.close();
	}

	public static void shutdown() {
		RikkaUi ui = current;
		if (ui == null) {
			return;
		}
		current = null;
		RikkaClient.events().unsubscribe(ui.hud);
		for (EmbeddedSurface surface : ui.surfaces) {
			surface.close();
		}
		ui.surfaces.clear();
		ui.embedding.close();
	}

	private static RikkaUi get() {
		RikkaUi ui = current;
		if (ui != null) {
			return ui;
		}
		RenderSystem.assertOnRenderThread();
		VulkanDevice device = (VulkanDevice) ((FrontendGpuDeviceAccessor) RenderSystem.getDevice()).rikka$backend();
		Embedding embedding = new Embedding(RikkaVulkan.describe(device));
		String failure = embedding.getErrorMessage();
		if (!failure.isEmpty()) {
			embedding.close();
			throw new IllegalStateException("Rikka UI cannot draw on Minecraft's Vulkan device: " + failure);
		}
		ui = new RikkaUi(embedding);
		current = ui;
		RikkaClient.LOGGER.info("Rikka UI draws on Minecraft's Vulkan device");
		return ui;
	}

	private EmbeddedSurface open(EmbeddedSurfaceBuilder surface) {
		EmbeddedSurface opened = this.embedding.open(surface);
		this.surfaces.add(opened);
		return opened;
	}
}
