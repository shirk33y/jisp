export const Component = ({foo}) => {
  const scope = useScope ();
  const render = useJisp (() => import ('./example.jisp'));

  const componentState = scope.useState ({open: true, title: 'foo'});
  const items = scope.useSelect ().homepage.items;
  const {loadItems, cleanup} = scope.useActions ();

  useEffect (() => {
    const process = loadItems ();

    return () => {
      cleanup ();
    };
  });

  return render(items);
};

const jisp = [
  'component',
  'Foo',
  ['title', 'href', 'items'],
  [
    'h1',
    'font-8',
    'md:px-2',
    {href: 'href'},
    ['div', 'px-8', 'md:px-2'],
    [
      'ul',
      'm-8',
      'text-white',
      'bg-gray-500',
      [
        'map',
        'items',
        ['item', 'idx'],
        [
          'li',
          'px-5',
          'hover:bg-blue-300',
          {text: 'Welcome to '},
          {text: 'item.name'},
          {text: ' my friend!'},
        ],
      ],
    ],
  ],
];
