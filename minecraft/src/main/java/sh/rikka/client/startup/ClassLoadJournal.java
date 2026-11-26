package sh.rikka.client.startup;

import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.stream.Stream;
import javax.management.JMException;
import javax.management.ObjectName;

final class ClassLoadJournal {
	private static final String CURRENT = "classes.log";
	private static final String PREVIOUS = "classes.previous.log";
	private static final String DIAGNOSTIC_COMMANDS = "com.sun.management:type=DiagnosticCommand";
	private static final String LOG_OPERATION = "vmLog";
	private static final String SOURCE_MARKER = " source: ";
	private static final String RUNTIME_IMAGE = "jrt:";
	private static final String SHARED_ARCHIVE = "shared objects file";
	private static final char HIDDEN_CLASS_MARKER = '/';

	private final Path current;
	private final Path previous;

	ClassLoadJournal(Path directory) {
		this.current = directory.resolve(CURRENT);
		this.previous = directory.resolve(PREVIOUS);
	}

	List<String> rotate() throws IOException {
		Files.createDirectories(this.current.getParent());
		if (Files.exists(this.current)) {
			Files.move(this.current, this.previous, StandardCopyOption.REPLACE_EXISTING);
		}
		if (!Files.exists(this.previous)) {
			return List.of();
		}
		try (Stream<String> lines = Files.lines(this.previous)) {
			return lines.map(ClassLoadJournal::gameClass).filter(name -> !name.isEmpty()).toList();
		}
	}

	void record() throws JMException {
		String[] arguments = {"output=\"" + this.current + '"', "what=class+load=info", "decorators=none", "output_options=filecount=0"};
		ManagementFactory.getPlatformMBeanServer()
			.invoke(new ObjectName(DIAGNOSTIC_COMMANDS), LOG_OPERATION, new Object[] {arguments}, new String[] {String[].class.getName()});
	}

	private static String gameClass(String line) {
		int source = line.indexOf(SOURCE_MARKER);
		if (source <= 0) {
			return "";
		}
		String name = line.substring(0, source);
		String origin = line.substring(source + SOURCE_MARKER.length());
		if (name.indexOf(HIDDEN_CLASS_MARKER) >= 0 || origin.startsWith(RUNTIME_IMAGE) || origin.startsWith(SHARED_ARCHIVE)) {
			return "";
		}
		return name;
	}
}
