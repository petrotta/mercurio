package io.github.petrotta.mercurio.interactive;

import io.github.petrotta.mercurio.commands.*;
import jline.console.ConsoleReader;
import picocli.CommandLine;

import java.io.PrintWriter;
import java.lang.Package;

import static io.github.petrotta.mercurio.Application.console;

@CommandLine.Command(name = "", description = "Example interactive shell with completion",
        footer = {"", "Press Ctrl-C to exit."}, subcommands = {Create.class, Evaluate.class, Inspect.class,  Run.class, Test.class, Validate.class, Version.class, Viz.class})
public class InteractiveCommandHandler implements Runnable {
    final ConsoleReader reader;
    final PrintWriter out;

    @CommandLine.Spec
    private CommandLine.Model.CommandSpec spec;

    public InteractiveCommandHandler(ConsoleReader reader) {
        this.reader = reader;
        out = new PrintWriter(reader.getOutput());
    }

    public void run() {
     //   console("42");
       out.println(spec.commandLine().getUsageMessage());
    }
}

