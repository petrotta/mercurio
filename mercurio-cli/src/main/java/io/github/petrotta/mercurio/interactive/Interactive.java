package io.github.petrotta.mercurio.interactive;

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.build.Project;
import io.github.petrotta.mercurio.commands.Create;
import io.github.petrotta.mercurio.commands.Evaluate;
import io.github.petrotta.mercurio.commands.ProjectCommand;
import jline.TerminalFactory;
import jline.console.ConsoleReader;
import jline.console.completer.ArgumentCompleter;
import jline.internal.Configuration;
import net.sourceforge.plantuml.tim.stdlib.Eval;
import picocli.CommandLine;
import picocli.shell.jline2.PicocliJLineCompleter;

import java.io.File;
import java.util.concurrent.Callable;


@CommandLine.Command(name = "interactive", description = "Example interactive shell with completion",
        footer = {"", "Press Ctrl-C to exit."})
public class Interactive extends ProjectCommand implements Callable<Integer> {

    @CommandLine.Option(names = { "-dir", "--dir" }, paramLabel = "SOURCE", description = "the source files or directories", arity = "0..1")
    private File sourceDir;

    @CommandLine.Option(names = { "-lib", "--lib" }, paramLabel = "LIBRARY", description = "the library directory", arity = "0..1")
    private File libDir;

    @CommandLine.Option(names = { "-verbose", "-v" }, paramLabel = "VERBOSE", description = "turn on verbose output", arity = "0..1", defaultValue = "false")
    private boolean verbose;

    @Override
    public Integer call() throws Exception { // your business logic goes here...



        startInteractiveShell();

        return 0;
    }


    public static void startInteractiveShell() {

        // JLine 2 does not detect some terminal as not ANSI compatible (e.g  Eclipse Console)
        // See : https://github.com/jline/jline2/issues/185
        // This is an optional workaround which allow to use picocli heuristic instead :
        if (!CommandLine.Help.Ansi.AUTO.enabled() && //
                Configuration.getString(TerminalFactory.JLINE_TERMINAL, TerminalFactory.AUTO).toLowerCase()
                        .equals(TerminalFactory.AUTO)) {
            TerminalFactory.configure(TerminalFactory.Type.NONE);
        }

        try {
            ConsoleReader reader = new ConsoleReader();
            CommandLine.IFactory factory = new CustomFactory(new InteractiveParameterConsumer(reader));

            // set up the completion
            InteractiveCommandHandler commands = new InteractiveCommandHandler(reader);

            CommandLine cmd = new CommandLine(commands, factory);



            reader.addCompleter(new PicocliJLineCompleter(cmd.getCommandSpec()));

            // start the shell and process input until the user quits with Ctrl-D
            String line;
            while ((line = reader.readLine("sysml> ")) != null) {
                ArgumentCompleter.ArgumentList list = new ArgumentCompleter.WhitespaceArgumentDelimiter()
                        .delimit(line, line.length());
                new CommandLine(commands, factory)
                        .execute(list.getArguments());
            }
        } catch (Throwable t) {
            t.printStackTrace();
        }
    }



}
