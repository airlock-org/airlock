# Platform Layer

Provides shared Nix modules, schemas, and builders.

Platform code is the reusable vocabulary the other lifecycle layers consume. It
should not encode one image's bootstrap defaults or one runtime's desired state.
