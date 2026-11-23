# Enhanced GitHub marketplace catalog

Source files for the public catalog at https://github.com/dmor-me/yapfast-plugin-registry.
The catalog uses the existing Yapfast registry format and SHA-256/commit checks.
Its first entry is the existing upstream Ping example, pinned to the reviewed
plugin branch commit. No plugin executes while browsing.

The catalog uses the root of its dedicated repository. Set
YAPFAST_PLUGIN_REGISTRY to another HTTPS registry URL when developing or
testing a catalog.
The same catalog can list plugins maintained in many separate public GitHub repos.
This does not automatically search all GitHub repositories or imply their safety.

Validate each listing with:
`yapfast-plugins verify-entry marketplace/plugins/com.example.ping.json`

Plugin creators submit one entry under plugins/ and add its ID to index.json.
Include an immutable commit, module hash, accurate permissions and source/license.
A maintainer reviews the source before merging; validation is not a security audit.
