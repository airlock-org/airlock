# Bootstrap Layer

Defines the first-boot bridge between an image and a manageable runtime.

This layer owns the generated config-domain defaults and the initial system
configuration imported by the image profile. Ongoing desired state belongs in
`runtime/`.
