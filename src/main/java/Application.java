
import picocli.CommandLine;

@CommandLine.Command()
public class Application {
    public static void main(String... args) {

        CommandLine commandLine = new CommandLine(new Application())
                .addSubcommand("validate",   new Validate())
                .addSubcommand("eval",   new Evaluate());

        int exitCode = commandLine.execute(args );


        System.exit(exitCode);
    }

}


