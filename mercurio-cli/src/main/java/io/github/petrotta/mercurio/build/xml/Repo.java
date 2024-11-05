package io.github.petrotta.mercurio.build.xml;

import com.fasterxml.jackson.dataformat.xml.annotation.JacksonXmlRootElement;
import lombok.Getter;
import lombok.Setter;

@JacksonXmlRootElement( localName = "git")
public class Repo {
    @Getter @Setter private String url, branch;
}
