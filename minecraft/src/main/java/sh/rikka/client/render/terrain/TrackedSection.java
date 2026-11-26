package sh.rikka.client.render.terrain;

import net.minecraft.client.SectionUpdateTracker;
import org.jspecify.annotations.Nullable;

public interface TrackedSection {
	boolean rikka$tracks(long sectionNode, int epoch);

	SectionUpdateTracker.@Nullable SectionDirtyState rikka$dirtyState();

	void rikka$track(long sectionNode, int epoch, SectionUpdateTracker.@Nullable SectionDirtyState state);
}
