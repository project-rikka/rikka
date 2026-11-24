package sh.rikka.client.mixin;

import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.multiplayer.chat.GuiMessage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import sh.rikka.client.mods.translate.ChatTranslateHooks;

@Mixin(ChatComponent.class)
abstract class ChatComponentMixin {
	@ModifyVariable(
		method = "addMessage",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/client/gui/components/ChatComponent;logChatMessage(Lnet/minecraft/client/multiplayer/chat/GuiMessage;)V",
			shift = At.Shift.AFTER
		)
	)
	private GuiMessage rikka$decorate(GuiMessage message) {
		return ChatTranslateHooks.decorate(message);
	}
}
