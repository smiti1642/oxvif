// Generic offline verification adapter. No official schema data is embedded.
import java.io.ByteArrayInputStream;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import javax.xml.XMLConstants;
import javax.xml.parsers.DocumentBuilderFactory;
import javax.xml.transform.Source;
import javax.xml.transform.dom.DOMSource;
import javax.xml.transform.stream.StreamSource;
import javax.xml.validation.SchemaFactory;
import org.w3c.dom.bootstrap.DOMImplementationRegistry;
import org.w3c.dom.ls.DOMImplementationLS;
import org.w3c.dom.ls.LSInput;
import org.xml.sax.ErrorHandler;
import org.xml.sax.SAXException;
import org.xml.sax.SAXParseException;

class SchemaVerifier {
    private static final int MAX_BYTES = 8 * 1024 * 1024;
    private static final ErrorHandler STRICT_ERRORS = new ErrorHandler() {
        public void warning(SAXParseException e) throws SAXException { throw e; }
        public void error(SAXParseException e) throws SAXException { throw e; }
        public void fatalError(SAXParseException e) throws SAXException { throw e; }
    };

    private static byte[] boundedRead(Path path) throws Exception {
        if (!Files.isRegularFile(path) || Files.size(path) > MAX_BYTES) {
            throw new IllegalArgumentException("invalid input file");
        }
        byte[] data = Files.readAllBytes(path);
        if (data.length > MAX_BYTES) throw new IllegalArgumentException("oversized input");
        return data;
    }

    private static void verify(Path working, boolean validate) throws Exception {
        Map<String, Path> catalogue = new HashMap<>();
        for (String line : Files.readAllLines(working.resolve("catalogue.tsv"))) {
            String[] fields = line.split("\t", -1);
            if (fields.length != 2) throw new IllegalArgumentException("invalid catalogue row");
            Path path = Path.of(fields[1]).toAbsolutePath().normalize();
            catalogue.put(fields[0], path);
            catalogue.put(fields[0].replaceFirst("https:", "http:"), path);
            catalogue.put(path.toUri().toString(), path);
        }
        DOMImplementationLS dom = (DOMImplementationLS) DOMImplementationRegistry.newInstance().getDOMImplementation("LS");
        SchemaFactory factory = new org.apache.xerces.jaxp.validation.XMLSchema11Factory();
        factory.setFeature(XMLConstants.FEATURE_SECURE_PROCESSING, true);
        factory.setFeature("http://apache.org/xml/features/validation/schema-full-checking", true);
        factory.setErrorHandler(STRICT_ERRORS);
        org.w3c.dom.ls.LSResourceResolver resolver = (type, namespace, publicId, systemId, base) -> {
            try {
                if (systemId == null) throw new IllegalArgumentException("no explicit resource location");
                URI location = base == null ? URI.create(systemId) : URI.create(base).resolve(systemId);
                String key = location.toString();
                if ("file".equals(location.getScheme())) {
                    key = Path.of(location).toAbsolutePath().normalize().toUri().toString();
                }
                Path file = catalogue.get(key);
                if (file == null) throw new IllegalArgumentException("unlisted resource");
                LSInput input = dom.createLSInput();
                input.setSystemId(file.toUri().toString());
                input.setBaseURI(file.toUri().toString());
                input.setByteStream(new ByteArrayInputStream(boundedRead(file)));
                return input;
            } catch (Exception e) {
                // Never return null: doing so requests the default resolver.
                throw new IllegalArgumentException("offline resolver refusal", e);
            }
        };
        factory.setResourceResolver(resolver);
        List<Source> sources = new ArrayList<>();
        for (String line : Files.readAllLines(working.resolve("roots.tsv"))) {
            String[] fields = line.split("\t", -1);
            if (fields.length != 2) throw new IllegalArgumentException("invalid schema row");
            sources.add(new StreamSource(new ByteArrayInputStream(boundedRead(Path.of(fields[1]))), fields[0]));
        }
        if (sources.isEmpty()) throw new IllegalArgumentException("no schema roots");
        var schema = factory.newSchema(sources.toArray(Source[]::new));
        if (!validate) {
            System.out.println("PASS: Xerces strict XSD 1.1 compilation; roots=" + sources.size() + "; no instance validation performed");
            return;
        }

        DocumentBuilderFactory documents = DocumentBuilderFactory.newInstance();
        documents.setNamespaceAware(true);
        documents.setFeature(XMLConstants.FEATURE_SECURE_PROCESSING, true);
        documents.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
        documents.setFeature("http://xml.org/sax/features/external-general-entities", false);
        documents.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
        documents.setXIncludeAware(false);
        int count = 0;
        for (String line : Files.readAllLines(working.resolve("cases.tsv"))) {
            String[] fields = line.split("\t", -1);
            if (fields.length < 2 || fields.length > 10) throw new IllegalArgumentException("invalid instance row");
            var builder = documents.newDocumentBuilder();
            builder.setErrorHandler(STRICT_ERRORS);
            builder.setEntityResolver((publicId, systemId) -> { throw new SAXException("external entity refused"); });
            var document = builder.parse(new ByteArrayInputStream(boundedRead(Path.of(fields[1]))));
            var element = document.getDocumentElement();
            String namespace = element.getNamespaceURI();
            String name = (namespace == null || namespace.isEmpty() ? "" : "{" + namespace + "}") + element.getLocalName();
            if (!name.equals(fields[0])) throw new IllegalArgumentException("unexpected instance root");
            var validator = schema.newValidator();
            validator.setErrorHandler(STRICT_ERRORS);
            validator.setResourceResolver(resolver);
            validator.validate(new DOMSource(document));
            if (fields.length > 2) {
                org.w3c.dom.Element payload = element;
                for (int step = 2; step < fields.length; step++) {
                    org.w3c.dom.Element selected = null;
                    int matches = 0, children = 0;
                    for (var child = payload.getFirstChild(); child != null; child = child.getNextSibling()) {
                        if (!(child instanceof org.w3c.dom.Element)) continue;
                        children++;
                        String childNs = child.getNamespaceURI();
                        String childName = (childNs == null || childNs.isEmpty() ? "" : "{" + childNs + "}") + child.getLocalName();
                        if (childName.equals(fields[step])) { matches++; selected = (org.w3c.dom.Element) child; }
                    }
                    if (matches != 1 || (step == fields.length - 1 && children != 1)) {
                        throw new IllegalArgumentException("missing or ambiguous payload path");
                    }
                    payload = selected;
                }
                // A separate root validation prevents a lax wrapper wildcard
                // from hiding an undeclared operation payload.
                validator.validate(new DOMSource(payload));
            }
            count++;
        }
        if (count == 0) throw new IllegalArgumentException("no instances");
        System.out.println("PASS: Xerces strict XSD 1.1 instances=" + count + "; not semantic conformance");
    }

    public static void main(String[] args) {
        try {
            if (args.length != 2 || !(args[1].equals("compile") || args[1].equals("validate"))) {
                throw new IllegalArgumentException("invalid invocation");
            }
            verify(Path.of(args[0]), args[1].equals("validate"));
        } catch (Exception error) {
            // Validator messages may contain source excerpts or user input.
            String category = error.getClass().getSimpleName();
            if (error instanceof SAXParseException && error.getMessage() != null) {
                String key = error.getMessage().split(":", 2)[0];
                if (key.matches("[A-Za-z0-9_.-]{1,80}")) category += ":" + key;
            }
            System.err.println("FAIL: Xerces " + category);
            System.exit(1);
        }
    }
}
