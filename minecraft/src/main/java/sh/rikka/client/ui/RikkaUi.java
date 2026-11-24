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
import os.rikka.ThemeBackground;
import sh.rikka.client.RikkaClient;
import sh.rikka.client.config.Preferences;
import sh.rikka.client.mixin.FrontendGpuDeviceAccessor;

public final class RikkaUi {
	private static @Nullable RikkaUi current;

	private final Embedding embedding;
	private final Set<EmbeddedSurface> surfaces = Collections.newSetFromMap(new IdentityHashMap<>());
	private final HudLayer hud;

	private RikkaUi(Embedding embedding) {
		this.embedding = embedding;
		Preferences preferences = RikkaClient.preferences();
		wear(preferences);
		preferences.theme().onChange(slug -> wearPreferences());
		preferences.background().onChange(background -> wearPreferences());
		this.hud = new HudLayer(this::open, RikkaClient.mods(), preferences.hud());
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
		ui.hud.draw(RikkaVulkan.image(target), minecraft.getWindow());
	}

	public static void drawScreen(RenderTarget target) {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft.gui.screen() instanceof RikkaScreen screen) {
			screen.draw(RikkaVulkan.image(target), minecraft.getWindow());
		}
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

	private static void wearPreferences() {
		RikkaUi ui = current;
		if (ui != null) {
			ui.wear(RikkaClient.preferences());
		}
	}

	private void wear(Preferences preferences) {
		this.embedding.setTheme(preferences.theme().get());
		this.embedding.setBackground(switch (preferences.background().get()) {
			case DESKTOP -> ThemeBackground.DESKTOP;
			case SOLID -> ThemeBackground.SOLID;
			case GLASS -> ThemeBackground.GLASS;
			case LIQUID -> ThemeBackground.LIQUID;
		});
	}

	private EmbeddedSurface open(EmbeddedSurfaceBuilder surface) {
		EmbeddedSurface opened = this.embedding.open(surface);
		this.surfaces.add(opened);
		return opened;
	}
}
