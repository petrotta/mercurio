package io.github.petrotta.mercurio.interactive;

import jline.console.ConsoleReader;
import picocli.CommandLine;

import java.io.PrintWriter;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "interactive", description = "Example interactive shell with completion",
        footer = {"", "Press Ctrl-C to exit."})
public class CliCommands implements Runnable {
    final ConsoleReader reader;
    final PrintWriter out;

    @CommandLine.Spec
    private CommandLine.Model.CommandSpec spec;

    public CliCommands(ConsoleReader reader) {
        this.reader = reader;
        out = new PrintWriter(reader.getOutput());
    }

    public void run() {
        console("42");
       out.println(spec.commandLine().getUsageMessage());
    }
}

