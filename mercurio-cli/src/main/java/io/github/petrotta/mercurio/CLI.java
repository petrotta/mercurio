package io.github.petrotta.mercurio;

import io.github.petrotta.mercurio.commands.*;
import io.github.petrotta.mercurio.commands.Package;
import io.github.petrotta.mercurio.interactive.CliCommands;
import io.github.petrotta.mercurio.utils.Logging;
import jline.console.ConsoleReader;


import org.apache.log4j.ConsoleAppender;
import org.apache.log4j.Logger;
import picocli.CommandLine;

import java.io.IOException;


@CommandLine.Command()
public class CLI {


    public static void main(String... args) throws IOException {

        ConsoleAppender ca = Logging.createConsoleAppender();
        Logger.getRootLogger().addAppender(ca);

        CommandLine commandLine = createCommandLine();

        int exitCode = commandLine.execute(args );
        System.exit(exitCode);
    }

    public static CommandLine createCommandLine() throws IOException {
        CommandLine commandLine = new CommandLine(new CLI())
                .addSubcommand("validate",  new Validate())
                .addSubcommand("eval",      new Evaluate())
                .addSubcommand("version",   new Version())
                .addSubcommand("create",    new Create())
                .addSubcommand("package",   new Package())
                .addSubcommand("run",       new Run())
                .addSubcommand("inspect",   new Inspect())
                .addSubcommand("interactive", new CliCommands(new ConsoleReader()));
        return commandLine;
    }
}
