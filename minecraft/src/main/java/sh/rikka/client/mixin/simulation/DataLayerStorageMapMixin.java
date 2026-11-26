package sh.rikka.client.mixin.simulation;

import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import net.minecraft.world.level.chunk.DataLayer;
import net.minecraft.world.level.lighting.DataLayerStorageMap;
import org.jspecify.annotations.Nullable;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import sh.rikka.client.simulation.light.LightSectionLedger;
import sh.rikka.client.simulation.light.LightSectionSnapshot;
import sh.rikka.client.simulation.light.SnapshotLayerStorage;

@Mixin(DataLayerStorageMap.class)
abstract class DataLayerStorageMapMixin implements SnapshotLayerStorage {
	@Shadow
	@Final
	protected Long2ObjectOpenHashMap<DataLayer> map;

	@Unique
	private @Nullable LightSectionSnapshot rikka$view;

	@Unique
	private @Nullable LightSectionLedger rikka$ledger;

	@Override
	public LightSectionSnapshot rikka$publish() {
		return this.rikka$ledger().publish(this.map);
	}

	@Override
	public void rikka$view(LightSectionSnapshot snapshot) {
		this.rikka$view = snapshot;
	}

	@Unique
	private LightSectionLedger rikka$ledger() {
		LightSectionLedger ledger = this.rikka$ledger;
		if (ledger == null) {
			ledger = new LightSectionLedger();
			this.rikka$ledger = ledger;
		}
		return ledger;
	}

	@Redirect(
		method = "getLayer",
		at = @At(value = "INVOKE", target = "Lit/unimi/dsi/fastutil/longs/Long2ObjectOpenHashMap;get(J)Ljava/lang/Object;")
	)
	private @Nullable Object rikka$layer(Long2ObjectOpenHashMap<DataLayer> layers, long sectionNode) {
		LightSectionSnapshot view = this.rikka$view;
		if (view != null) {
			return view.get(sectionNode);
		}
		return layers.get(sectionNode);
	}

	@Redirect(
		method = {"setLayer", "copyDataLayer"},
		at = @At(value = "INVOKE", target = "Lit/unimi/dsi/fastutil/longs/Long2ObjectOpenHashMap;put(JLjava/lang/Object;)Ljava/lang/Object;")
	)
	private @Nullable Object rikka$put(Long2ObjectOpenHashMap<DataLayer> layers, long sectionNode, Object layer) {
		this.rikka$ledger().record(sectionNode);
		return layers.put(sectionNode, (DataLayer) layer);
	}

	@Redirect(
		method = "removeLayer",
		at = @At(value = "INVOKE", target = "Lit/unimi/dsi/fastutil/longs/Long2ObjectOpenHashMap;remove(J)Ljava/lang/Object;")
	)
	private @Nullable Object rikka$remove(Long2ObjectOpenHashMap<DataLayer> layers, long sectionNode) {
		this.rikka$ledger().record(sectionNode);
		return layers.remove(sectionNode);
	}
}
