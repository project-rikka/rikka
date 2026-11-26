package sh.rikka.client.simulation.light;

import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import it.unimi.dsi.fastutil.longs.LongOpenHashSet;
import net.minecraft.world.level.chunk.DataLayer;

public final class LightSectionLedger {
	private static final int RETAINED_CHANGES = 1024;

	private LongOpenHashSet changed = new LongOpenHashSet();
	private LightSectionSnapshot published = LightSectionSnapshot.EMPTY;
	private long[] directoryStamps = new long[LightSectionSnapshot.EMPTY.directoryCount()];
	private long[] pageStamps = new long[LightSectionSnapshot.EMPTY.pageCount()];
	private long generation;

	public void record(final long sectionNode) {
		this.changed.add(sectionNode);
	}

	public LightSectionSnapshot publish(final Long2ObjectOpenHashMap<DataLayer> source) {
		if (this.changed.isEmpty()) {
			return this.published;
		}
		this.generation++;
		if (this.published.fits(source.size())) {
			this.published = this.published.with(this.changed, source, this.directoryStamps, this.pageStamps, this.generation);
		} else {
			this.published = LightSectionSnapshot.of(source);
			this.directoryStamps = new long[this.published.directoryCount()];
			this.pageStamps = new long[this.published.pageCount()];
		}
		if (this.changed.size() > RETAINED_CHANGES) {
			this.changed = new LongOpenHashSet();
		} else {
			this.changed.clear();
		}
		return this.published;
	}
}
