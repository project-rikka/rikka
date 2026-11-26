package sh.rikka.client.simulation.light;

public interface SnapshotLayerStorage {
	LightSectionSnapshot rikka$publish();

	void rikka$view(LightSectionSnapshot snapshot);
}
