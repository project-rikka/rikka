package sh.rikka.client.mods.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.resources.language.I18n;
import net.minecraft.world.entity.player.Inventory;
import os.rikka.Color;
import os.rikka.Element;
import os.rikka.FontWeight;
import os.rikka.View;
import sh.rikka.client.hud.style.HudLook;
import sh.rikka.client.mod.HudCanvas;
import sh.rikka.client.mod.OverlayHudMod;
import sh.rikka.client.setting.BoolSetting;
import sh.rikka.client.setting.ScreenPosition;

public final class InventoryDisplayMod extends OverlayHudMod {
	private static final int COLUMNS = Inventory.getSelectionSize();
	private static final int ROWS = 3;
	private static final int FIRST_SLOT = COLUMNS;
	private static final float SLOT = 18.0F * UNITS_PER_GUI_PIXEL;
	private static final float ITEM = 16.0F * UNITS_PER_GUI_PIXEL;
	private static final float PADDING = 6.0F;
	private static final float TITLE_HEIGHT = 20.0F;
	private static final float TITLE_TEXT_SIZE = 12.0F;
	private static final float SLOT_RADIUS = 4.0F;
	private static final float SLOT_INSET = 1.0F;
	private static final Color SLOT_FILL = new Color(1.0F, 1.0F, 1.0F, 0.07F);

	private final BoolSetting title = bool("title", true);
	private final BoolSetting background = bool("background", true);
	private final BoolSetting slots = bool("slots", true);

	public InventoryDisplayMod(ScreenPosition defaultPosition) {
		super("inventory_display", defaultPosition, "inventory_2");
	}

	@Override
	public float width() {
		return COLUMNS * SLOT + PADDING * 2.0F;
	}

	@Override
	public float height() {
		return gridTop() + ROWS * SLOT + PADDING;
	}

	@Override
	protected void view(View view, HudLook look) {
		Element panel = view.box(() -> {
		}).position(0.0F, 0.0F).width(width()).height(height());
		if (this.background.get()) {
			look.panel(panel);
		}
		if (this.title.get()) {
			look.text(view.text(I18n.get(translationKey() + ".title"))
				.position(PADDING + SLOT_INSET, PADDING)
				.textSize(TITLE_TEXT_SIZE)
				.textWeight(FontWeight.MEDIUM)
				.textNoWrap());
		}
		if (this.slots.get()) {
			for (int row = 0; row < ROWS; row++) {
				for (int column = 0; column < COLUMNS; column++) {
					view.box(() -> {
					})
						.position(slotX(column) + SLOT_INSET, slotY(row) + SLOT_INSET)
						.width(SLOT - SLOT_INSET * 2.0F)
						.height(SLOT - SLOT_INSET * 2.0F)
						.cornerRadius(SLOT_RADIUS)
						.background(SLOT_FILL);
				}
			}
		}
	}

	@Override
	protected void extract(HudCanvas canvas) {
		LocalPlayer player = Minecraft.getInstance().player;
		if (player == null) {
			return;
		}
		Inventory inventory = player.getInventory();
		float inset = (SLOT - ITEM) / 2.0F;
		for (int row = 0; row < ROWS; row++) {
			for (int column = 0; column < COLUMNS; column++) {
				canvas.item(inventory.getItem(FIRST_SLOT + row * COLUMNS + column), slotX(column) + inset, slotY(row) + inset, ITEM);
			}
		}
	}

	private float gridTop() {
		return this.title.get() ? PADDING + TITLE_HEIGHT : PADDING;
	}

	private static float slotX(int column) {
		return PADDING + column * SLOT;
	}

	private float slotY(int row) {
		return gridTop() + row * SLOT;
	}
}
