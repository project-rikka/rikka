package sh.rikka.client.mixin;

import java.util.List;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.multiplayer.chat.GuiMessage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(ChatComponent.class)
public interface ChatComponentAccessor {
	@Accessor("allMessages")
	List<GuiMessage> rikka$allMessages();

	@Accessor("chatScrollbarPos")
	int rikka$chatScrollbarPos();

	@Accessor("chatScrollbarPos")
	void rikka$setChatScrollbarPos(int position);

	@Accessor("newMessageSinceScroll")
	boolean rikka$newMessageSinceScroll();

	@Accessor("newMessageSinceScroll")
	void rikka$setNewMessageSinceScroll(boolean unread);

	@Invoker("refreshTrimmedMessages")
	void rikka$refreshTrimmedMessages();
}
