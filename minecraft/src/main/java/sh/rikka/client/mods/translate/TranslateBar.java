package sh.rikka.client.mods.translate;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.ComponentPath;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.narration.NarrationElementOutput;
import net.minecraft.client.gui.navigation.FocusNavigationEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.input.MouseButtonInfo;
import net.minecraft.network.chat.Component;
import org.jspecify.annotations.Nullable;

public final class TranslateBar extends AbstractWidget {
	private static final int HEIGHT = 12;
	private static final int INPUT_HEIGHT = 14;
	private static final int GAP = 2;
	private static final int MARGIN = 2;
	private static final int PADDING = 4;
	private static final int TEXT_TOP = 2;
	private static final int TEXT = 0xFFFFFFFF;
	private static final int TEXT_MUTED = 0xFFA0A0A0;
	private static final int HOVER = 0x30FFFFFF;

	private final ChatTranslateMod mod;
	private final Font font;
	private final int screenWidth;
	private Component toggleLabel = Component.empty();
	private Component languageLabel = Component.empty();
	private int toggleWidth;
	private @Nullable Shown shown;

	TranslateBar(ChatTranslateMod mod, Font font, int screenWidth, int screenHeight) {
		super(0, screenHeight - INPUT_HEIGHT - GAP - HEIGHT, 0, HEIGHT, Component.empty());
		this.mod = mod;
		this.font = font;
		this.screenWidth = screenWidth;
		layout();
	}

	public boolean scrolled(double x, double y, double amount) {
		if (amount == 0.0 || !this.visible || !isMouseOver(x, y) || region(x) != Region.LANGUAGE) {
			return false;
		}
		this.mod.cycleOutgoingLanguage(amount > 0.0 ? -1 : 1);
		return true;
	}

	@Override
	protected void extractWidgetRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
		layout();
		int background = Minecraft.getInstance().options.getBackgroundColor(Integer.MIN_VALUE);
		Region hovered = this.isHovered ? region(mouseX) : null;
		int languageLeft = getX() + this.toggleWidth + GAP;
		chip(graphics, getX(), this.toggleWidth, background, hovered == Region.TOGGLE);
		chip(graphics, languageLeft, getRight() - languageLeft, background, hovered == Region.LANGUAGE);
		graphics.text(this.font, this.toggleLabel, getX() + PADDING, getY() + TEXT_TOP, TEXT);
		graphics.text(this.font, this.languageLabel, languageLeft + PADDING, getY() + TEXT_TOP, this.mod.translatesOutgoing() ? TEXT : TEXT_MUTED);
		if (hovered != null) {
			graphics.setTooltipForNextFrame(this.font, Texts.of(hovered == Region.TOGGLE ? "bar.outgoing.tooltip" : "bar.language.tooltip"), mouseX, mouseY);
		}
	}

	private void chip(GuiGraphicsExtractor graphics, int left, int width, int background, boolean hovered) {
		graphics.fill(left, getY(), left + width, getBottom(), background);
		if (hovered) {
			graphics.fill(left, getY(), left + width, getBottom(), HOVER);
		}
	}

	@Override
	public void onClick(MouseButtonEvent event, boolean doubleClick) {
		switch (region(event.x())) {
			case TOGGLE -> this.mod.toggleOutgoing();
			case LANGUAGE -> this.mod.cycleOutgoingLanguage(event.button() == InputConstants.MOUSE_BUTTON_RIGHT ? -1 : 1);
		}
	}

	@Override
	protected boolean isValidClickButton(MouseButtonInfo buttonInfo) {
		return buttonInfo.button() == InputConstants.MOUSE_BUTTON_LEFT || buttonInfo.button() == InputConstants.MOUSE_BUTTON_RIGHT;
	}

	@Override
	public boolean shouldTakeFocusAfterInteraction() {
		return false;
	}

	@Override
	public @Nullable ComponentPath nextFocusPath(FocusNavigationEvent navigationEvent) {
		return null;
	}

	@Override
	protected void updateWidgetNarration(NarrationElementOutput output) {
		defaultButtonNarrationText(output);
	}

	private void layout() {
		Shown current = new Shown(this.mod.translatesOutgoing(), this.mod.outgoingTarget());
		if (current.equals(this.shown)) {
			return;
		}
		this.shown = current;
		Component state = Texts.of(current.on() ? "bar.on" : "bar.off").withStyle(current.on() ? ChatFormatting.GREEN : ChatFormatting.RED);
		this.toggleLabel = Texts.of("bar.outgoing", state);
		this.languageLabel = Texts.of("bar.language", Texts.languageName(current.target()));
		this.toggleWidth = this.font.width(this.toggleLabel) + 2 * PADDING;
		int languageWidth = this.font.width(this.languageLabel) + 2 * PADDING;
		int width = this.toggleWidth + GAP + languageWidth;
		setWidth(width);
		setX(this.screenWidth - MARGIN - width);
		setMessage(Component.empty().append(this.toggleLabel).append(" ").append(this.languageLabel));
	}

	private Region region(double x) {
		return x < getX() + this.toggleWidth + GAP / 2.0 ? Region.TOGGLE : Region.LANGUAGE;
	}

	private record Shown(boolean on, String target) {
	}

	private enum Region {
		TOGGLE,
		LANGUAGE
	}
}
