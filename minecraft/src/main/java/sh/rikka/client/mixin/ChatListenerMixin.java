package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import java.util.Optional;
import net.minecraft.client.multiplayer.chat.ChatListener;
import net.minecraft.network.chat.ChatType;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.PlayerChatMessage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import sh.rikka.client.mods.translate.ChatTranslateHooks;

@Mixin(ChatListener.class)
abstract class ChatListenerMixin {
	private static final String DECORATE = "Lnet/minecraft/network/chat/ChatType$Bound;decorate(Lnet/minecraft/network/chat/Component;)Lnet/minecraft/network/chat/Component;";

	@WrapOperation(method = {"handlePlayerChatMessage", "showMessageToPlayer"}, at = @At(value = "INVOKE", target = DECORATE))
	private Component rikka$decoratePlayerChat(
		ChatType.Bound chatType, Component content, Operation<Component> original, @Local(argsOnly = true) PlayerChatMessage message
	) {
		Component decorated = original.call(chatType, content);
		ChatTranslateHooks.body(decorated, content, Optional.of(message.sender()));
		return decorated;
	}

	@WrapOperation(method = "lambda$handleDisguisedChatMessage$0", at = @At(value = "INVOKE", target = DECORATE))
	private Component rikka$decorateDisguisedChat(ChatType.Bound chatType, Component content, Operation<Component> original) {
		Component decorated = original.call(chatType, content);
		ChatTranslateHooks.body(decorated, content, Optional.empty());
		return decorated;
	}
}
