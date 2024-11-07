

import io.github.petrotta.mercurio.Application;
import io.github.petrotta.mercurio.CLI;
import io.github.petrotta.mercurio.commands.Validate;
import io.github.petrotta.mercurio.commands.Version;
import org.junit.jupiter.api.Test;
import picocli.CommandLine;

import java.io.File;
import java.io.IOException;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.util.concurrent.Callable;

import static org.junit.jupiter.api.Assertions.fail;

public class TestCommands {

    private static final String PILOT_SAMPLES = "C:/dev/git/SysML-v2-Pilot-Implementation/sysml/src";

    @Test
    public void runTests() throws IOException {

        runTest( CLI.createCommandLine() , new String[] {"version"});
        runTest( CLI.createCommandLine() , new String[] {"validate", "-dir", "tmp/test1"});
        runTest( CLI.createCommandLine() , new String[] {"validate", "-dir", PILOT_SAMPLES+ "/examples/Mass Roll-up Example"});
        runTest( CLI.createCommandLine() , new String[] {"validate", "-dir", PILOT_SAMPLES});

        runTest( CLI.createCommandLine() , new String[] {"inspect", "-dir", "tmp/test1"});

        runTest( CLI.createCommandLine() , new String[] {"inspect", "-dir", PILOT_SAMPLES});
        runTest( CLI.createCommandLine() , new String[] {"inspect", "-dir", PILOT_SAMPLES+ "/examples/Mass Roll-up Example"});

        runTest( CLI.createCommandLine() , new String[] {"eval", "-dir", PILOT_SAMPLES+ "/examples/Mass Roll-up Example", "-i", "A", "-t", "MassConstraintExample.Engine"});


        File newProject = new File("tmp/test"+System.currentTimeMillis());
        runTest( CLI.createCommandLine() , new String[] {"create", "-dir", newProject.getAbsolutePath() });
        runTest( CLI.createCommandLine() , new String[] {"validate", "-dir", newProject.getAbsolutePath() });


        runTest( CLI.createCommandLine() , new String[] {"run", "-dir", newProject.getAbsolutePath() });







    }



    private void runTest(CommandLine commandLine, String[] args) {

        System.out.println("******************************************************");
        System.out.println(commandLine.getClass().getSimpleName() + " Output: ");
        int exitCode = commandLine.execute(args);
        System.out.println("******************************************************");

        assert (exitCode == 0);
    }


}
