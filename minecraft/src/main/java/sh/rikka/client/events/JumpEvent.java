package sh.rikka.client.events;

import net.minecraft.client.player.LocalPlayer;
import sh.rikka.client.event.Event;

public record JumpEvent(LocalPlayer player) implements Event {
}
