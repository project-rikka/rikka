package sh.rikka.client.events;

import sh.rikka.client.event.Event;

public record ChunkLoadEvent(int chunkX, int chunkZ) implements Event {
}
