package sh.rikka.client.mods.visual;

import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.util.ARGB;
import sh.rikka.client.event.EventHandler;
import sh.rikka.client.events.TickEvent;
import sh.rikka.client.mixin.OverlayTextureAccessor;
import sh.rikka.client.mod.Category;
import sh.rikka.client.mod.Mod;
import sh.rikka.client.setting.ColorSetting;

public final class HitColorMod extends Mod {
	private static final int VANILLA_OVERLAY = 0xB2FF0000;
	private static final int TEXTURE_SIZE = 16;
	private static final int HURT_ROWS = 8;

	private final ColorSetting color = color("color", ARGB.color(0xFF - ARGB.alpha(VANILLA_OVERLAY), VANILLA_OVERLAY));
	private boolean stale = true;

	@SuppressWarnings("this-escape")
	public HitColorMod() {
		super("hit_color", Category.VISUAL, "colorize");
		this.color.onChange(value -> this.stale = true);
	}

	@Override
	protected void onEnable() {
		this.stale = true;
	}

	@Override
	protected void onDisable() {
		write(VANILLA_OVERLAY);
	}

	@EventHandler
	private void onTick(TickEvent.Pre event) {
		if (this.stale || this.color.get().chroma()) {
			int chosen = this.color.argb();
			if (write(ARGB.color(0xFF - ARGB.alpha(chosen), chosen))) {
				this.stale = false;
			}
		}
	}

	private static boolean write(int overlay) {
		GameRenderer renderer = Minecraft.getInstance().gameRenderer;
		if (renderer == null) {
			return false;
		}
		DynamicTexture texture = ((OverlayTextureAccessor) renderer.overlayTexture()).rikka$texture();
		NativeImage pixels = texture.getPixels();
		if (pixels == null) {
			return false;
		}
		for (int y = 0; y < HURT_ROWS; y++) {
			for (int x = 0; x < TEXTURE_SIZE; x++) {
				pixels.setPixel(x, y, overlay);
			}
		}
		texture.upload();
		return true;
	}
}
